"""kill_enemies.py [--write [--only <hex block addr>...]]: list (or, with --write, zero the AP of) every character in
the running mission but the player (or only the listed ones). Debug helper for test runs (e.g. to clear a mission quickly), from outside the game: ReadProcessMemory /
WriteProcessMemory, no Cheat Engine, no restart.

The player's AP block is [[[[exe+4E0F598]+A5A0]+320]+0] (AP i32 at +278, max at +27C, RE_NOTES "player AP"); every
other character has a block of the same class (same vtable at +0): the private RW memory is scanned for that vtable."""
import ctypes, ctypes.wintypes as wt, struct, sys

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

needle = struct.pack("<Q", vt)
found = []
addr, mbi = 0, MBI()
while k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
    start, size = mbi.BaseAddress or 0, mbi.RegionSize
    if mbi.State == 0x1000 and mbi.Protect & 0x44 and not mbi.Protect & 0x100 and mbi.Type != 0x1000000:  # committed, RW, no guard, not an image
        CHUNK = 0x4000000  # 64 MB reads (the game's big heap region is GBs); +0x1000 overlap for blocks across a cut
        for off in range(0, size, CHUNK):
            data = read(start + off, min(CHUNK + 0x1000, size - off))
            if not data: continue
            i = data.find(needle)
            while i != -1 and i < CHUNK:
                if i % 8 == 0 and i + AP_MAX + 4 <= len(data):
                    a, m = struct.unpack_from("<ii", data, i + AP)
                    if 0 < m <= 1_000_000 and 0 <= a <= m: found.append((start + off + i, a, m))
                i = data.find(needle, i + 1)
    addr = start + size
    if addr >= 0x7FFFFFFFFFFF: break

for obj, a, m in found:
    tag = "player" if obj == player else "other"
    print("%s %#x AP %d/%d" % (tag, obj, a, m))
only = [int(v, 16) for v in sys.argv[sys.argv.index("--only") + 1:]] if "--only" in sys.argv else None
if "--write" in sys.argv:
    for obj, a, m in found:
        if obj == player or a == 0 or (only is not None and obj not in only): continue
        ok = k32.WriteProcessMemory(h, ctypes.c_void_p(obj + AP), struct.pack("<i", 0), 4, None)
        print("AP 0 -> %#x: %s" % (obj, bool(ok)))
