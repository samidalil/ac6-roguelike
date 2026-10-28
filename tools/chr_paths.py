"""chr_paths.py: RE helper, from WorldChrMan find how every character's AP block (same class as the player's) is
reached: WCM+off -> ChrIns (ChrIns+0x320 -> modules, [modules] = AP block) or WCM+off -> array -> ChrIns. Read only;
run while in a mission."""
import ctypes, struct, sys
import numpy as np
sys.argv = [sys.argv[0]]
exec(open(__file__.replace("chr_paths.py", "kill_enemies.py")).read().split("player = ")[0])
wcm = q(base + WORLDCHRMAN)
pchr = q(wcm + WCM_PLAYER)
player = q(q(pchr + PLAYER_MODULES))
vt = q(player)
print("WCM %#x player chr %#x block %#x" % (wcm, pchr, player))

def block_of(c):
    m = q(c + PLAYER_MODULES)
    if not m: return None
    b = q(m)
    if not b or q(b) != vt: return None
    a, mx = struct.unpack("<ii", read(b + AP, 8) or b"\0" * 8)
    return (b, a, mx) if 0 < mx <= 1_000_000 else None

buf = read(wcm, 0x20000)
words = np.frombuffer(buf, dtype="<u8")
for i, p in enumerate(words):
    p = int(p)
    if p < 0x10000 or p > 0x7FFFFFFFFFFF or p & 7: continue
    r = block_of(p)
    if r: print("WCM+%#x -> chr %#x -> block %#x AP %d/%d" % (i * 8, p, *r))
    arr = read(p, 0x800)
    if not arr: continue
    for j, c in enumerate(np.frombuffer(arr[: len(arr) // 8 * 8], dtype="<u8")):
        c = int(c)
        if c < 0x10000 or c > 0x7FFFFFFFFFFF or c & 7: continue
        r = block_of(c)
        if r: print("WCM+%#x -> %#x +%#x -> chr %#x -> block %#x AP %d/%d" % (i * 8, p, j * 8, c, *r))
