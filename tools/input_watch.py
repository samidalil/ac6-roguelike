"""input_watch.py <seconds> <hex addr>...: poll 32-byte windows around addresses of the running game (every 50 ms) and
print each change with a timestamp (RE of the game's input state; see input_scan.py). Read only."""
import ctypes, sys, time
sys.argv, args = [sys.argv[0]], sys.argv[1:]
src = open(__file__.replace("input_watch.py", "kill_enemies.py")).read().split("mods = ")[0]
exec(src)  # find_pid / OpenProcess / read() from kill_enemies.py
secs, addrs = float(args[0]), [int(a, 16) & ~0xF for a in args[1:]]
addrs = sorted(set(addrs))
last = {}
end = time.time() + secs
while time.time() < end:
    for a in addrs:
        b = read(a - 16, 48)
        if b is None: continue
        if last.get(a) != b:
            if a in last:
                old = last[a]
                diffs = ["+%d:%02x>%02x" % (i - 16, old[i], b[i]) for i in range(48) if old[i] != b[i]]
                print("%s %#x %s" % (time.strftime("%H:%M:%S"), a, " ".join(diffs)), flush=True)
            last[a] = b
    time.sleep(0.05)
