"""ptr_scan.py <hex target> [max offset] [depth]: find pointer chains from the exe's static data to <target> in the
running game (pointers to anything in [target - max offset, target]), breadth-first up to <depth> levels. Read only.
Prints chains as exe+RVA -> +off -> +off ... (the last offset lands on the target)."""
import ctypes, struct, sys
import numpy as np
sys.argv, args = [sys.argv[0]], sys.argv[1:]
exec(open(__file__.replace("ptr_scan.py", "kill_enemies.py")).read().split("player = ")[0])
target, maxoff, depth = int(args[0], 16), int(args[1], 0) if len(args) > 1 else 0x800, int(args[2]) if len(args) > 2 else 3

def regions():
    out, addr, mbi = [], 0, MBI()
    while k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
        s, z = mbi.BaseAddress or 0, mbi.RegionSize
        if mbi.State == 0x1000 and mbi.Protect in (0x04, 0x08, 0x40, 0x80): out.append((s, z))
        addr = s + z
        if addr >= 0x7FFFFFFFFFFF: break
    return out

image_end = base + struct.unpack("<I", read(base + struct.unpack("<I", read(base + 0x3C, 4))[0] + 0x50, 4))[0]  # SizeOfImage
snap = []
for s, z in regions():
    for off in range(0, z, 0x4000000):
        n = min(0x4000000, z - off)
        b = read(s + off, n)
        if b and len(b) >= 8: snap.append((s + off, np.frombuffer(b[: len(b) // 8 * 8], dtype="<u8")))

def refs(lo, hi):
    out = []
    for s, a in snap:
        idx = np.nonzero((a >= lo) & (a <= hi))[0]
        for i in idx[:2000]: out.append((s + int(i) * 8, int(a[i])))
    return out

level = {target: []}  # address -> chain of offsets from it to the target
found = 0
for d in range(depth):
    nxt = {}
    for t, chain in level.items():
        for where, val in refs(t - maxoff, t):
            c = [t - val] + chain
            if base <= where < image_end:
                print("exe+%X -> %s" % (where - base, " -> ".join("+%X" % o for o in c)), flush=True)
                found += 1
            elif where not in nxt and len(nxt) < 20000:
                nxt[where] = c
    level = nxt
    print("level %d: %d heap pointer(s)" % (d + 1, len(level)), flush=True)
    if found: break
