"""CE-free monitor of the owned-parts set (CSEquipmentLineupData+0xA0) and the flag-bypass map (+0x08): logs any
change in membership/count with a timestamp. Catches mission part GRANTS (inventory), which are separate from shop
release-wave flags. Usage: AC6_PID=<pid> python owned_monitor.py"""
import ctypes, struct, sys, time, os
from ac6_lib import PID, BASE
K = ctypes.windll.kernel32; h = K.OpenProcess(0x0410, False, PID); assert h
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def q(a):
    b = rpm(a, 8); return struct.unpack("<Q", b)[0] if len(b) == 8 else 0
def u32(a):
    b = rpm(a, 4); return struct.unpack("<I", b)[0] if len(b) == 4 else 0
LOG = os.path.join(os.environ["USERPROFILE"], "ac6_owned_monitor.log"); out = open(LOG, "a", buffering=1)
def log(s):
    line = time.strftime("%H:%M:%S ") + s; print(line); out.write(line + "\n")
def walk(hdr, vs):
    keys = set()
    def rec(n, d=0):
        if d > 90 or n == 0 or rpm(n + 0x19, 1) != b"\x00": return
        rec(q(n), d + 1); v = rpm(n + 0x20, vs); keys.add((u32(n + 0x1C), v[0] if vs == 1 else 0)); rec(q(n + 0x10), d + 1)
    rec(q(hdr + 8)); return keys
def snapshot():
    ld = q(q(BASE + 0x4E03A08) + 0x40)
    if not ld: return None, None
    owned = {k for k, _ in walk(q(ld + 0xA0 + 0x20), 8)}
    bypass = {k for k, v in walk(q(ld + 8), 1) if v == 1}
    return owned, bypass
po, pb = snapshot()
log("start pid=%d owned=%s bypass=%s" % (PID, len(po) if po is not None else None, len(pb) if pb is not None else None))
while True:
    time.sleep(0.5)
    o, b = snapshot()
    if o is None: continue
    if po is not None and o != po:
        for k in sorted(o - po): log("OWNED +  0x%08X (equip %d)  [owned now %d]" % (k, k & 0x0FFFFFFF, len(o)))
        for k in sorted(po - o): log("OWNED -  0x%08X (equip %d)  [owned now %d]" % (k, k & 0x0FFFFFFF, len(o)))
    if pb is not None and b != pb:
        for k in sorted(b - pb): log("BYPASS + 0x%08X" % k)
        for k in sorted(pb - b): log("BYPASS - 0x%08X" % k)
    po, pb = o, b
