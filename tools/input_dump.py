"""input_dump.py <hex addr> <len> [seconds]: hexdump a range of the running game, then (with seconds) print each changed
4-byte word (offset, old, new, as hex / i32 / f32) every 30 ms for that long. Read only."""
import ctypes, struct, sys, time
sys.argv, args = [sys.argv[0]], sys.argv[1:]
exec(open(__file__.replace("input_dump.py", "kill_enemies.py")).read().split("mods = ")[0])
base, n = int(args[0], 16), int(args[1], 0)
secs = float(args[2]) if len(args) > 2 else 0
b = read(base, n)
for i in range(0, n, 16):
    words = struct.unpack_from("<4I", b, i)
    print("%#x +%03x  %s" % (base + i, i, " ".join("%08x" % w for w in words)))
last = b
end = time.time() + secs
while time.time() < end:
    b = read(base, n)
    if b and b != last:
        for i in range(0, n, 4):
            o, w = last[i:i + 4], b[i:i + 4]
            if o != w:
                print("%s +%03x %s -> %s (i32 %d, f32 %.3f)" % (time.strftime("%H:%M:%S"), i, o.hex(), w.hex(),
                      struct.unpack("<i", w)[0], struct.unpack("<f", w)[0]), flush=True)
        last = b
    time.sleep(0.03)
