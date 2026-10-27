"""CE-free event-flag transition monitor. Polls the CSEventFlagMan bit pages for flag ids 0..9999 every ~200 ms via
ReadProcessMemory (no breakpoints -> zero interference with the game) and logs every 0<->1 transition with a timestamp.
Use it to learn WHICH flags flip when a mission unlocks parts; then set a one-shot write-watch on that byte for a backtrace.
Usage: AC6_PID=<pid> python flag_monitor.py [logfile]   (Ctrl+C to stop)"""
import ctypes, struct, sys, time, os
from ac6_lib import PID, BASE
K = ctypes.windll.kernel32
h = K.OpenProcess(0x0410, False, PID); assert h
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def q(a):
    b = rpm(a, 8); return struct.unpack("<Q", b)[0] if len(b) == 8 else 0
def u32(a):
    b = rpm(a, 4); return struct.unpack("<I", b)[0] if len(b) == 4 else 0
man = q(BASE + 0x4E13070); div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
def page_for(block):
    hdr = q(man + 0x38); node = q(hdr + 8); best = hdr
    guard = 0
    while rpm(node + 0x19, 1) != b"\x01" and guard < 128:
        guard += 1
        if u32(node + 0x20) < block: node = q(node + 0x10)
        else: best = node; node = q(node)
    if best == hdr or u32(best + 0x20) != block: return None
    typ = u32(best + 0x28)
    return bbase + bsz * u32(best + 0x30) if typ == 1 else (q(best + 0x30) if typ == 2 else None)
LOG = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.environ["USERPROFILE"], "ac6_flag_monitor.log")
out = open(LOG, "a", buffering=1)
def log(s):
    line = time.strftime("%H:%M:%S ") + ("%.3f " % (time.time() % 60)) + s
    print(line); out.write(line + "\n")
BLOCKS = list(range(0, 10))   # flag ids 0..9999
pages = {b: page_for(b) for b in BLOCKS}
log("monitor start pid=%d man=%X div=%d bsz=%d pages=%s" % (PID, man, div, bsz, {b: ("%X" % p if p else None) for b, p in pages.items()}))
prev = {}
for b, p in pages.items():
    if p: prev[b] = rpm(p, bsz)
first = True
while True:
    for b, p in pages.items():
        if not p: continue
        cur = rpm(p, bsz)
        if len(cur) != bsz: continue
        old = prev.get(b)
        if old and cur != old:
            for i in range(bsz):
                if cur[i] != old[i]:
                    diff = cur[i] ^ old[i]
                    for bit in range(8):
                        if diff & (0x80 >> bit):
                            fid = b * div + i * 8 + bit
                            state = 1 if cur[i] & (0x80 >> bit) else 0
                            log("FLAG %d -> %d" % (fid, state))
        prev[b] = cur
    time.sleep(0.2)
