"""CE-free monitor of the item manager's DEFER state and PENDING-OBTAINED list (RE 2026-09-24).
mgr = GameData+0x600.  mgr+0x99 = defer byte (AddItem core exe+474D80 pushes the 4-byte item id onto the pending
vector at mgr+0x78 {alloc@0, begin@8, end@10, cap@18} via exe+477170 when it is 1 and returns -1; the mission-end
"obtained parts" step (commit core exe+4756E0) shows the list, inserts on OK and clears the byte).
Logs every change of: defer byte, pending count/ids, flag 5074 (mission cleared), COAM (GameData+0x74).  ~200 ms polling, zero interference.
Usage: python pending_monitor.py [logfile]"""
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
def u8(a):
    b = rpm(a, 1); return b[0] if b else -1
def flag(fid):
    """IsEventFlag(fid) read straight from CSEventFlagMan's bit pages (re-resolved every call, survives New Game)."""
    man = q(BASE + 0x4E13070)
    if not man: return -1
    div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
    if not div: return -1
    block, rem = divmod(fid, div)
    hdr = q(man + 0x38); node = q(hdr + 8); best = hdr; guard = 0
    while rpm(node + 0x19, 1) != b"\x01" and guard < 128:
        guard += 1
        if u32(node + 0x20) < block: node = q(node + 0x10)
        else: best = node; node = q(node)
    if best == hdr or u32(best + 0x20) != block: return -1
    typ = u32(best + 0x28)
    page = bbase + bsz * u32(best + 0x30) if typ == 1 else (q(best + 0x30) if typ == 2 else 0)
    if not page: return -1
    byte = u8(page + (rem >> 3))
    return (byte >> (7 - (rem & 7))) & 1 if byte >= 0 else -1
LOG = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.environ["USERPROFILE"], "ac6_pending_monitor.log")
out = open(LOG, "a", buffering=1)
def log(s):
    line = time.strftime("%H:%M:%S ") + ("%.3f " % (time.time() % 60)) + s
    print(line); out.write(line + "\n")
log("pending monitor start pid=%d" % PID)
prev = None
while True:
    gdm = q(BASE + 0x4E03A08); gd = q(gdm + 8) if gdm else 0
    if gd:
        mgr = gd + 0x600
        d = u8(mgr + 0x99); b, e = q(mgr + 0x80), q(mgr + 0x88)
        n = (e - b) // 4 if 0 < e - b < 0x1000 else 0
        ids = [u32(b + 4 * i) for i in range(min(n, 8))]
        coam = struct.unpack("<i", struct.pack("<I", u32(gd + 0x74)))[0]
        cur = (gd, d, n, tuple(ids), flag(5074), coam)
    else:
        cur = (0, -1, 0, (), -1, 0)
    if cur != prev:
        log("GameData=%X defer(+99)=%d pending=%d ids=%s flag5074=%d COAM=%d" % (cur[0], cur[1], cur[2], " ".join("%08X" % i for i in cur[3]), cur[4], cur[5]))
        prev = cur
    time.sleep(0.2)
