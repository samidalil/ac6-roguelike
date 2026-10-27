"""CE-free snapshot of the shop state: evaluates the per-frame release predicate (exe+159ACB0) for every
EquipmentLineupParam row using the reimplemented CSEventFlagMan lookup + the flag-bypass map, and scans memory for
every shop container (vtable exe+395CE18) / 0x118-stride entry (vtable exe+390BA08).  Read-only."""
import ctypes, ctypes.wintypes as w, struct, sys
from ac6_lib import PID, BASE, SIZE
K = ctypes.windll.kernel32
h = K.OpenProcess(0x0410, False, PID); assert h
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
q = lambda a: struct.unpack("<Q", rpm(a, 8))[0]
u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]
s32 = lambda a: struct.unpack("<i", rpm(a, 4))[0]
man = q(BASE + 0x4E13070); div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
def page_for(block):
    hdr = q(man + 0x38); node = q(hdr + 8); best = hdr
    while rpm(node + 0x19, 1) != b"\x01":
        if u32(node + 0x20) < block: node = q(node + 0x10)
        else: best = node; node = q(node)
    if best == hdr or u32(best + 0x20) != block: return None
    typ = u32(best + 0x28)
    return bbase + bsz * u32(best + 0x30) if typ == 1 else (q(best + 0x30) if typ == 2 else None)
def getflag(fid):
    p = page_for(fid // div); sub = fid % div
    return None if p is None else int(bool(rpm(p + (sub >> 3), 1)[0] & (1 << (7 - (sub & 7)))))
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD), ("RegionSize", ctypes.c_size_t), ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]
K.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
def regions():
    addr = 0
    while addr < 0x7FFFFFFFFFFF:
        m = MBI()
        if not K.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)): break
        if m.State == 0x1000 and (m.Protect & 0xFF) in (2, 4, 8, 0x20, 0x40) and not (BASE <= (m.BaseAddress or 0) < BASE + SIZE):
            yield m.BaseAddress, m.RegionSize
        addr = (m.BaseAddress or 0) + m.RegionSize
table = None; conts = []; entries = []
VT_C = struct.pack("<Q", BASE + 0x395CE18); VT_E = struct.pack("<Q", BASE + 0x390BA08)
for ba, sz in regions():
    b = rpm(ba, sz)
    if not b: continue
    j = b.find(b"EQUIPMENT_LINEUP_PARAM\0")
    if j != -1 and table is None: table = ba + j - 318 * 0x20
    for needle, out in ((VT_C, conts), (VT_E, entries)):
        i = b.find(needle)
        while i != -1:
            if i % 8 == 0: out.append(ba + i)
            i = b.find(needle, i + 1)
print("lineup table data start: %X" % table)
print("containers with vtable exe+395CE18: %d" % len(conts))
for c in conts:
    b, e, cap = q(c + 0x18), q(c + 0x20), q(c + 0x28)
    n = (e - b) // 0x118 if e >= b and (e - b) < 0x400000 else -1
    print("   %X  begin=%X end=%X cap=%X  count=%d  refcount=%d" % (c, b, e, cap, n, q(c + 8)))
    if 0 < n <= 12:
        for i in range(n):
            x = b + i * 0x118
            print("      entry %d @%X gaitem=0x%08X price=%d ord=%d row=%X s68=%d s6C=%d s70=%d b74=%d" % (i, x, u32(x + 0x54), s32(x + 0x20), s32(x + 0x58), q(x + 0x60), s32(x + 0x68), s32(x + 0x6C), s32(x + 0x70), rpm(x + 0x74, 1)[0]))
print("qwords == exe+390BA08 (entry vtable) anywhere: %d" % len(entries))
ents = sorted(a - 0x10 for a in entries)
runs = []
for a in ents:
    if runs and a == runs[-1][1]: runs[-1][1] = a + 0x118
    else: runs.append([a, a + 0x118])
print("entry runs (>=2): %s" % [("%X" % r[0], (r[1] - r[0]) // 0x118) for r in runs if (r[1] - r[0]) // 0x118 >= 2][:10])
print("single entries: %s" % ["%X" % r[0] for r in runs if (r[1] - r[0]) // 0x118 == 1][:20])
# ---- predicate evaluation
ld = q(q(BASE + 0x4E03A08) + 0x40); bypass = set()
def walk(n):
    if rpm(n + 0x19, 1) != b"\x00": return
    walk(q(n))
    if rpm(n + 0x20, 1)[0]: bypass.add(u32(n + 0x1C))
    walk(q(n + 0x10))
walk(q(q(ld + 8) + 8))
print("bypass ids (%d): %s" % (len(bypass), ["0x%08X" % x for x in sorted(bypass)]))
rows = [(table + i * 0x20) for i in range(318)]
vis = []; byflag = {}
for r in rows:
    eq, price, flag, typ, rew = u32(r), s32(r + 4), s32(r + 8), rpm(r + 0xC, 1)[0], s32(r + 0x10)
    cat = {0: 1, 1: 1, 4: 4, 5: 7, 6: 8}  # not needed for the check; key = typ<<28|eq per notes for frames (0x1.. prefix)
    gid_candidates = {eq, (0x1 << 28) | eq, (0x7 << 28) | eq}
    if gid_candidates & bypass: res, why = 1, "bypass"
    elif rew > 0: res, why = 0, "reward"
    elif flag in (0, -1): res, why = 0, "flag0"
    else:
        f = getflag(flag); res, why = (f or 0), ("evflag%d=%s" % (flag, f))
    byflag.setdefault((flag, why if not why.startswith("evflag") else why.split("=")[1]), 0)
    byflag[(flag, why if not why.startswith("evflag") else why.split("=")[1])] += 1
    if res: vis.append((eq, typ, price, flag))
print("rows predicate says VISIBLE: %d of 318" % len(vis))
print("by type: %s" % {t: len([1 for v in vis if v[1] == t]) for t in sorted(set(v[1] for v in vis))})
print("per (flag, outcome) counts: %s" % dict(sorted(byflag.items())))
print("first visible weapons: %s" % [("0x%08X" % v[0], v[2]) for v in vis if v[1] == 0][:12])
