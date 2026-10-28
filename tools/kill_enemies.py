"""kill_enemies.py [--write [--only <hex block addr>...]] [--max-ap N] [--scan]: list (or, with --write, zero the AP of) every
character in the running mission but the player (or only the listed ones, or only those whose max AP is N: one pass,
e.g. --max-ap 12300 for the tester AC). Debug helper for test runs (e.g. to clear a mission quickly), from outside the
game: ReadProcessMemory / WriteProcessMemory, no Cheat Engine, no restart.

The player's AP block is [[[[exe+4E0F598]+A5A0]+320]+0] (AP i32 at +278, max at +27C, RE_NOTES "player AP"); every
other character has a block of the same class (same vtable at +0), reached through WorldChrMan's character lists (or, with
--scan, by scanning the game's private RW memory for that vtable)."""
import ctypes, ctypes.wintypes as wt, struct, sys, time
import numpy as np

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)
WORLDCHRMAN, WCM_PLAYER, PLAYER_MODULES, AP, AP_MAX = 0x4E0F598, 0xA5A0, 0x320, 0x278, 0x27C

class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", wt.DWORD),
                ("PartitionId", wt.WORD), ("RegionSize", ctypes.c_size_t), ("State", wt.DWORD), ("Protect", wt.DWORD),
                ("Type", wt.DWORD)]

def find_pid(name="armoredcore6.exe"):
    arr = (wt.DWORD * 4096)(); got = wt.DWORD()
    psapi.EnumProcesses(arr, ctypes.sizeof(arr), ctypes.byref(got))
    for pid in arr[:got.value // 4]:
        h = k32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not h: continue
        buf = ctypes.create_unicode_buffer(512); n = wt.DWORD(512)
        ok = k32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)); k32.CloseHandle(h)
        if ok and buf.value.lower().endswith(name): return pid
    return None

pid = find_pid()
if not pid: sys.exit("game not running")
h = k32.OpenProcess(0x0010 | 0x0020 | 0x0008 | 0x0400, False, pid)  # VM_READ | VM_WRITE | VM_OPERATION | QUERY_INFORMATION
if not h: sys.exit("OpenProcess failed: %d" % ctypes.get_last_error())

def read(addr, n):
    buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
    if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)): return None
    return buf.raw[:got.value]
def q(addr):
    b = read(addr, 8)
    return struct.unpack("<Q", b)[0] if b and len(b) == 8 else 0

mods = (ctypes.c_void_p * 1024)(); need = wt.DWORD()
psapi.EnumProcessModulesEx(h, mods, ctypes.sizeof(mods), ctypes.byref(need), 3)
base = mods[0]
player = q(q(q(q(base + WORLDCHRMAN) + WCM_PLAYER) + PLAYER_MODULES))
if not player: sys.exit("no player character (not in a mission?)")
vt = q(player)
ap, mx = struct.unpack("<ii", read(player + AP, 8))
print("player block %#x vtable %#x AP %d/%d" % (player, vt, ap, mx))

# Characters are listed from WorldChrMan (found in the tester AC mission with tools/chr_paths.py): [WCM+0xAB58] = the
# player and the opposing ACs (tester AC = entry 1), [WCM+0xAB78] / [WCM+0xA760] = scripted objects. Each entry is a
# ChrIns whose [+0x320] modules -> [0] AP block. --scan searches the whole memory instead (slow, ~45 s).
CHR_LISTS = (0xAB58, 0xAB78, 0xA760)
found = []
t0 = time.time()
def block_of(c):
    b = q(q(c + PLAYER_MODULES)) if c else 0
    if not b or q(b) != vt: return None
    a, m = struct.unpack("<ii", read(b + AP, 8))
    return (b, a, m) if 0 < m <= 1_000_000 and 0 <= a <= m else None
if "--scan" not in sys.argv:
    wcm = q(base + WORLDCHRMAN)
    for off in CHR_LISTS:
        arr = q(wcm + off)
        for k in range(16):
            r = block_of(q(arr + 8 * k)) if arr else None
            if r is None: break
            if all(r[0] != f[0] for f in found): found.append(r)
addr, mbi = 0, MBI()
while "--scan" in sys.argv and k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
    start, size = mbi.BaseAddress or 0, mbi.RegionSize
    if mbi.State == 0x1000 and mbi.Protect & 0x44 and not mbi.Protect & 0x100 and mbi.Type != 0x1000000:  # committed, RW, no guard, not an image
        CHUNK = 0x4000000  # 64 MB reads (the game's big heap region is GBs); blocks are 8-aligned, AP at +0x278
        for off in range(0, size, CHUNK):
            data = read(start + off, min(CHUNK + 0x1000, size - off))
            if not data: continue
            words = np.frombuffer(data[: len(data) // 8 * 8], dtype="<u8")
            for i in np.nonzero(words == vt)[0] * 8:
                if i >= CHUNK or i + AP_MAX + 4 > len(data): continue
                a, m = struct.unpack_from("<ii", data, int(i) + AP)
                if 0 < m <= 1_000_000 and 0 <= a <= m: found.append((start + off + int(i), a, m))
    addr = start + size
    if addr >= 0x7FFFFFFFFFFF: break

for obj, a, m in found:
    tag = "player" if obj == player else "other"
    print("%s %#x AP %d/%d" % (tag, obj, a, m))
only = [int(v, 16) for v in sys.argv[sys.argv.index("--only") + 1:] if not v.startswith("--")] if "--only" in sys.argv else None
max_ap = int(sys.argv[sys.argv.index("--max-ap") + 1]) if "--max-ap" in sys.argv else None
print("found in %.1f s" % (time.time() - t0))
if "--write" in sys.argv or max_ap is not None:
    for obj, a, m in found:
        if obj == player or a == 0 or (only is not None and obj not in only) or (max_ap is not None and m != max_ap): continue
        ok = k32.WriteProcessMemory(h, ctypes.c_void_p(obj + AP), struct.pack("<i", 0), 4, None)
        print("AP 0 -> %#x: %s" % (obj, bool(ok)))
