"""Live test: clear a shop entry's release event flag while the Parts Shop buy list is open, watch the per-frame list,
then restore the flag to its ORIGINAL state.  CE-free.  Usage: python flag_test.py [entry_index] [--set-flag ID VALUE]"""
import ctypes, ctypes.wintypes as w, struct, sys, time
from ac6_lib import PID, BASE, SIZE
K = ctypes.windll.kernel32
h = K.OpenProcess(0x0438, False, PID); assert h, "OpenProcess (read/write) failed"
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def wpm(a, data):
    r = ctypes.c_size_t(); return bool(K.WriteProcessMemory(h, ctypes.c_void_p(a), data, len(data), ctypes.byref(r)))
q = lambda a: struct.unpack("<Q", rpm(a, 8))[0]
u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]
s32 = lambda a: struct.unpack("<i", rpm(a, 4))[0]
# ---- CSEventFlagMan (exe+4E13070), layout per exe+B07CF0 / exe+B08500
man = q(BASE + 0x4E13070); div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
def page_for(block):
    hdr = q(man + 0x38); node = q(hdr + 8); best = hdr
    while rpm(node + 0x19, 1) != b"\x01":
        if u32(node + 0x20) < block: node = q(node + 0x10)
        else: best = node; node = q(node)
    if best == hdr or u32(best + 0x20) != block: return None
    typ = u32(best + 0x28)
    return bbase + bsz * u32(best + 0x30) if typ == 1 else (q(best + 0x30) if typ == 2 else None)
def bitaddr(fid):
    p = page_for(fid // div); sub = fid % div
    return (p + (sub >> 3), 1 << (7 - (sub & 7))) if p else (None, None)
def getflag(fid):
    a, m = bitaddr(fid); return None if a is None else int(bool(rpm(a, 1)[0] & m))
def setflag(fid, st):
    a, m = bitaddr(fid); b = rpm(a, 1)[0]; b = (b | m) if st else (b & ~m & 0xFF); return wpm(a, bytes([b]))
if "--set-flag" in sys.argv:
    i = sys.argv.index("--set-flag"); fid, val = int(sys.argv[i+1]), int(sys.argv[i+2])
    print("flag %d: %s -> set %d -> %s" % (fid, getflag(fid), val, (setflag(fid, val), getflag(fid))[1])); sys.exit(0)
# ---- live EquipmentLineupParam table: type string "EQUIPMENT_LINEUP_PARAM" - 318*0x20
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD), ("RegionSize", ctypes.c_size_t), ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]
K.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
def regions():
    addr = 0
    while addr < 0x7FFFFFFFFFFF:
        m = MBI()
        if not K.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)): break
        if m.State == 0x1000 and m.Protect in (4, 8) and not (BASE <= (m.BaseAddress or 0) < BASE + SIZE): yield m.BaseAddress, m.RegionSize
        addr = (m.BaseAddress or 0) + m.RegionSize
containers, table = [], None
# container vtables: detail list (exe+395CE18) and the big buy list; --container ADDR adds one explicitly
vts = {BASE + 0x395CE18}
if "--container" in sys.argv:
    c0 = int(sys.argv[sys.argv.index("--container") + 1], 16); v0 = q(c0)
    if BASE <= v0 < BASE + SIZE: vts.add(v0); print("container %X has vtable exe+%X" % (c0, v0 - BASE))
needles = [struct.pack("<Q", v) for v in vts]
for ba, sz in regions():
    b = rpm(ba, sz)
    for vt in needles:
        i = b.find(vt)
        while i != -1:
            if i % 8 == 0 and 0 <= q(ba + i + 0x20) - q(ba + i + 0x18) < 0x200000: containers.append(ba + i)
            i = b.find(vt, i + 1)
    j = b.find(b"EQUIPMENT_LINEUP_PARAM\0")
    if j != -1 and table is None: table = ba + j - 318 * 0x20
assert table, "lineup table not found"
# fallback discovery: runs of 0x118-stride entries (entry vtable exe+390BA08 at +0x10) -> owner whose +0x18 == run start
if not any(len(range(q(c + 0x18), q(c + 0x20), 0x118)) > 1 for c in containers):
    evt = struct.pack("<Q", BASE + 0x390BA08); hits = []
    for ba, sz in regions():
        b = rpm(ba, sz); i = b.find(evt)
        while i != -1:
            if i % 8 == 0: hits.append(ba + i - 0x10)
            i = b.find(evt, i + 1)
    hits.sort(); runs = []
    for a in hits:
        if runs and a == runs[-1][1]: runs[-1][1] = a + 0x118
        else: runs.append([a, a + 0x118])
    runs = [r for r in runs if (r[1] - r[0]) // 0x118 >= 2]
    print("entry runs:", [("%X" % r[0], (r[1] - r[0]) // 0x118) for r in runs][:6])
    for r0, r1 in runs:
        nd = struct.pack("<Q", r0)
        for ba, sz in regions():
            b = rpm(ba, sz); i = b.find(nd)
            while i != -1:
                if i % 8 == 0 and i >= 0x18 and q(ba + i + 8) <= r1 and q(ba + i + 8) >= r0: containers.append(ba + i - 0x18); print("owner of run %X: container %X (vt exe+%X)" % (r0, ba + i - 0x18, (q(ba + i - 0x18) - BASE) & 0xFFFFFFFF))
                i = b.find(nd, i + 1)
rows = {}
for i in range(318):
    r = table + i * 0x20; rows[u32(r)] = r
def lineup_row(gaitem): return rows.get(gaitem & 0x0FFFFFFF) or rows.get(gaitem)
def snap(c):
    b, e = q(c + 0x18), q(c + 0x20); n = (e - b) // 0x118 if e >= b else 0
    return [(u32(b + i * 0x118 + 0x54)) for i in range(n)]
live = sorted(((len(snap(c)), c) for c in containers), reverse=True)
print("containers:", [("%X" % c, n) for n, c in live])
if not live or live[0][0] == 0: print("no live shop list: open the Parts Shop buy list first"); sys.exit(1)
C = live[0][1]; ids = snap(C)
# flag-bypass map (CSEquipmentLineupData+0x08): entries in it ignore flags, so skip them
ld = q(q(BASE + 0x4E03A08) + 0x40); bypass = set()
def walk(n):
    if rpm(n + 0x19, 1) == b"": return
    walk(q(n));
    if rpm(n + 0x20, 1)[0]: bypass.add(u32(n + 0x1C))
    walk(q(n + 0x10))
walk(q(q(ld + 8) + 8)); print("flag-bypass ids:", ["0x%08X" % x for x in sorted(bypass)])
idx = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else next((i for i, x in enumerate(ids) if x not in bypass and lineup_row(x) and s32(lineup_row(x) + 8) >= 1000), 0)
g = ids[idx]; row = lineup_row(g)
flag = s32(row + 8) if row else None
share = [x for x in ids if lineup_row(x) and s32(lineup_row(x) + 8) == flag]
print("list %X: %d entries; entry[%d] gaitem=0x%08X lineup row=%X price=%d flag=%d reward=%d ; entries sharing flag: %d" % (C, len(ids), idx, g, row or 0, s32(row + 4), flag, s32(row + 0x10), len(share)))
if not flag or flag < 1000: print("entry has flag %s: pick another entry index" % flag); sys.exit(1)
orig = getflag(flag); print("flag %d original state: %d" % (flag, orig))
try:
    print("CLEAR ->", setflag(flag, 0)); time.sleep(0.7)
    ids2 = snap(C); print("after clear : isFlag=%d count=%d ; entries sharing flag still listed: %d ; entry 0x%08X still listed: %s" % (getflag(flag), len(ids2), len([x for x in ids2 if x in share]), g, g in ids2))
    time.sleep(0.8)
finally:
    print("RESTORE ->", setflag(flag, orig)); time.sleep(0.7)
    ids3 = snap(C); print("after restore: isFlag=%d count=%d" % (getflag(flag), len(ids3)))
