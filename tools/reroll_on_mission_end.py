"""Prototype of the mod's mission-end trigger: poll CSEventFlagMan for the result-screen close (flag 3139: 1 -> 0),
then reroll the predicate-hook cave's CHOSEN table with 1 random weapon + 2 random frame parts (unowned, buyable).
CE-free. Requires the CE hook (tools/ce/hook_shop_predicate.cea) to be enabled (cave at 0x13FFF0000).
Usage: AC6_PID=<pid> python reroll_on_mission_end.py"""
import ctypes, struct, re, os, random, time
import ctypes.wintypes as w
from ac6_lib import PID, BASE, SIZE
K = ctypes.windll.kernel32; h = K.OpenProcess(0x0438, False, PID); assert h
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def wpm(a, d):
    r = ctypes.c_size_t(); return bool(K.WriteProcessMemory(h, ctypes.c_void_p(a), d, len(d), ctypes.byref(r)))
q = lambda a: struct.unpack("<Q", rpm(a, 8))[0]; u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]; s32 = lambda a: struct.unpack("<i", rpm(a, 4))[0]
LOG = os.path.join(os.environ["USERPROFILE"], "ac6_reroll.log"); out = open(LOG, "a", buffering=1)
def log(s):
    line = time.strftime("%H:%M:%S ") + s; print(line); out.write(line + "\n")
# --- flag manager bit for id 3139
man = q(BASE + 0x4E13070); div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
def page_for(block):
    hdr = q(man + 0x38); node = q(hdr + 8); best = hdr; g = 0
    while rpm(node + 0x19, 1) != b"\x01" and g < 128:
        g += 1
        if u32(node + 0x20) < block: node = q(node + 0x10)
        else: best = node; node = q(node)
    typ = u32(best + 0x28); return bbase + bsz * u32(best + 0x30) if typ == 1 else q(best + 0x30)
def flag(fid):
    p = page_for(fid // div); s = fid % div; return 1 if rpm(p + (s >> 3), 1)[0] & (0x80 >> (s & 7)) else 0
# --- cave chosen table
CAVE = 0x13FFF0000
cave = rpm(CAVE, 0x200); i = cave.find(bytes.fromhex("90BD9800"))
if i == -1:  # table may already hold other ids: find "dd ...; dd 0" after the ret sequences -> locate by the 'xor al,al; ret' epilogue
    i = cave.find(b"\x30\xC0\xC3") + 3
TBL = CAVE + i
# --- live param pool
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD), ("RegionSize", ctypes.c_size_t), ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]
K.VirtualQueryEx.argtypes = [w.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
def find_param():
    needle = b"EQUIPMENT_LINEUP_PARAM\0"; addr = 0
    while addr < 0x7FFFFFFFFFFF:
        m = MBI()
        if not K.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)): break
        if m.State == 0x1000 and (m.Protect & 0xFF) in (4, 8) and not (BASE <= (m.BaseAddress or 0) < BASE + SIZE):
            b = rpm(m.BaseAddress, m.RegionSize); j = b.find(needle)
            while j != -1:
                d = m.BaseAddress + j - 318 * 0x20
                if u32(d + 74 * 0x20) == 50070400 and u32(d + 140 * 0x20) == 10000000: return d
                j = b.find(needle, j + 1)
        addr = (m.BaseAddress or 0) + m.RegionSize
N = r"D:\samee\Workspace\Modding\PC\From Software\DSMapStudio-1.10\Assets\Paramdex\AC6\Names"; names = {}
for fn in sorted(os.listdir(N)):
    if fn.lower().startswith("equipparam") and "npc" not in fn.lower():
        for line in open(N + "\\" + fn, encoding="utf-8", errors="replace"):
            mm = re.match(r"\s*(\d+)\s+(.*)", line)
            if mm: names.setdefault(int(mm.group(1)), mm.group(2).strip())
def owned_ids():
    gd = q(q(BASE + 0x4E03A08) + 0x08); tbl = q(gd + 0x600 + 0x10); s = set()
    for k in range(256):
        hd, idv = struct.unpack("<II", rpm(tbl + k * 16, 8))
        if hd == 0xFFFFFFFF: break
        s.add(idv & 0x0FFFFFFF)
    return s
# cache the param location + the buyable pool ONCE (the memory scan takes ~45 s; a reroll must be instant)
DATA = find_param(); POOL_W, POOL_F = [], []
for k in range(318):
    r = DATA + k * 0x20; eq = u32(r); pr = s32(r + 4); ty = rpm(r + 0xC, 1)[0]; rw = s32(r + 0x10)
    if pr > 0 and rw == 0: (POOL_W if ty == 0 else POOL_F if ty == 1 else []).append(eq)
def reroll(tag):
    t0 = time.time(); own = owned_ids(); cur = [u32(TBL + k * 4) for k in range(3)]
    weap = [e for e in POOL_W if e not in cur and (e & 0x0FFFFFFF) not in own]
    frm = [e for e in POOL_F if e not in cur and (e & 0x0FFFFFFF) not in own]
    new = [random.choice(weap)] + random.sample(frm, 2)
    ok = wpm(TBL, struct.pack("<III", *new))
    log("REROLL (%s) in %.2fs -> %s  written=%s" % (tag, time.time() - t0, [("0x%08X %s" % (x, names.get(x & 0x0FFFFFFF, "?"))) for x in new], ok))
log("watcher start pid=%d cave table @%X current=%s param=%X pool=%dW/%dF" % (PID, TBL, ["0x%08X" % u32(TBL + k * 4) for k in range(3)], DATA, len(POOL_W), len(POOL_F)))
import subprocess
BRIDGE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ce_bridge.py")
def grant_random_part(tag):
    own = owned_ids(); cands = [e for e in POOL_W if (e & 0x0FFFFFFF) not in own] + [0x10000000 | (e & 0x0FFFFFFF) for e in POOL_F if (e & 0x0FFFFFFF) not in own]
    g = random.choice(cands)
    r = subprocess.run(["python", BRIDGE, "local r = executeCodeEx(0, nil, getAddress('giveitem'), %d); ac6.log('watcher grant 0x%08X -> rax=%%s', tostring(r)); return 1" % (g, g)], capture_output=True, text=True, timeout=30)
    log("GRANT (%s) -> 0x%08X %s  (bridge rc=%s)" % (tag, g, names.get(g & 0x0FFFFFFF, "?"), r.returncode))
prev = flag(3139); prev74 = flag(5074)
log("flag 3139 initial=%d, 5074 initial=%d (armed: GRANT on 5074 1->0 = mission end; REROLL on 3139 1->0 = result closed)" % (prev, prev74))
while True:
    time.sleep(0.2)
    cur = flag(3139); cur74 = flag(5074)
    if prev74 == 1 and cur74 == 0:
        log("mission ended (5074 1->0) -> granting a random part BEFORE the result screen")
        try: grant_random_part("mission-end")
        except Exception as ex: log("grant failed: %r" % (ex,))
    if prev == 1 and cur == 0:
        log("mission result screen closed (3139 1->0) -> rerolling shop")
        reroll("mission-end")
    prev, prev74 = cur, cur74
