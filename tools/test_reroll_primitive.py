"""Decide the reroll primitive on an all-unlocked save.

Question: does setting an OWNED item's +0x08 bypass-map node to true force it into the
visible buy list (defeating the ownership filter)? If yes, reroll = byte writes to the
bypass map. Run with the Parts Shop BUY list (Achat) OPEN.

Usage: PYTHONIOENCODING=utf-8 AC6_PID=<pid> python test_reroll_primitive.py [equipId]
Reads/writes only single bytes in existing map nodes; restores them at the end.
"""
import ctypes, struct, sys, time
from ac6_lib import PID, BASE, SIZE
K = ctypes.windll.kernel32
h = K.OpenProcess(0x0438, False, PID); assert h, "OpenProcess (rw) failed"
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def wr1(a, v):
    r = ctypes.c_size_t(); return bool(K.WriteProcessMemory(h, ctypes.c_void_p(a), bytes([v]), 1, ctypes.byref(r)))
q = lambda a: struct.unpack("<Q", rpm(a, 8))[0]
u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]

ld = q(q(BASE + 0x4E03A08) + 0x40)
print("lineupData = %X" % ld)

# --- walk the +0x08 bool map, collecting (key, value_addr, value) for every node ---
def walk_map(hdr, vsize):
    nodes = []
    def rec(n, d=0):
        if d > 90 or rpm(n + 0x19, 1) != b"\x00":
            return
        rec(q(n), d + 1)
        nodes.append((u32(n + 0x1C), n + 0x20, rpm(n + 0x20, vsize)))
        rec(q(n + 0x10), d + 1)
    rec(q(hdr + 8))
    return nodes

boolmap = walk_map(q(ld + 0x08), 1)          # gaitemId -> bool
owned = {k for k, _, _ in walk_map(q(ld + 0xA0 + 0x20), 8)}
print("boolmap nodes: %d (true: %d)   owned-set: %d" %
      (len(boolmap), sum(1 for _, _, v in boolmap if v == b"\x01"), len(owned)))

by_key = {k: (addr, val) for k, addr, val in boolmap}

# --- find the live visible buy-list container (vtable exe+395CE18), stride 0x118 ---
import ctypes.wintypes as w
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD), ("RegionSize", ctypes.c_size_t),
                ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]
K.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
def containers():
    needle = struct.pack("<Q", BASE + 0x395CE18); out = []; addr = 0
    while addr < 0x7FFFFFFFFFFF:
        m = MBI()
        if not K.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)): break
        if m.State == 0x1000 and m.Protect in (4, 8) and not (BASE <= (m.BaseAddress or 0) < BASE + SIZE):
            b = rpm(m.BaseAddress, m.RegionSize); i = b.find(needle)
            while i != -1:
                if i % 8 == 0 and 0 <= q(m.BaseAddress + i + 0x20) - q(m.BaseAddress + i + 0x18) < 0x400000:
                    out.append(m.BaseAddress + i)
                i = b.find(needle, i + 1)
        addr = (m.BaseAddress or 0) + m.RegionSize
    return out
def snap(c):
    b, e = q(c + 0x18), q(c + 0x20); n = (e - b) // 0x118 if e >= b else 0
    return [u32(b + i * 0x118 + 0x54) & 0x0FFFFFFF for i in range(n)]

def best_container():
    cs = containers()
    live = sorted(((len(snap(c)), c) for c in cs), reverse=True)
    return (live[0][1], live[0][0]) if live else (None, 0)

C, n0 = best_container()
if not C:
    print("no live buy-list container — open the Parts Shop > Achat first"); sys.exit(1)
print("buy list container %X: %d entries" % (C, n0))

# pick an owned item that is NOT already bypass-true and IS a node in the map
target = int(sys.argv[1]) if len(sys.argv) > 1 else None
if target is None:
    for k, addr, val in boolmap:
        eq = k & 0x0FFFFFFF
        if val == b"\x00" and (k in owned or eq in owned):
            target = k; break
if target is None:
    print("could not pick an owned+bypass-false node"); sys.exit(1)
addr, val = by_key[target]
eq = target & 0x0FFFFFFF
print("target gaitem 0x%08X (equip %d) owned=%s bypass currently=%d @node %X" %
      (target, eq, target in owned or eq in owned, val[0], addr))

before = eq in snap(C)
print("before: in buy list = %s" % before)
print("SET bypass true ->", wr1(addr, 1)); time.sleep(0.7)
after = eq in snap(C)
print("after bypass=1: in buy list = %s  (count %d)" % (after, len(snap(C))))
time.sleep(0.6)
print("RESTORE bypass=%d ->" % val[0], wr1(addr, val[0])); time.sleep(0.5)
print("restored: in buy list = %s" % (eq in snap(C)))
print("\nVERDICT: bypass-map write %s defeat the ownership filter" %
      ("DOES" if (after and not before) else "does NOT"))
