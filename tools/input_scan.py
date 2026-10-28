"""input_scan.py: find the bytes of the running game that follow an input (RE of the game's own input state).

Snapshots of every committed private RW region are taken on command: write "R <delay s>" (input released) or
"H <delay s>" (input held) into %TEMP%\\ac6_scan_cmd.txt; the script waits <delay>, snapshots, and writes "done <n>"
into %TEMP%\\ac6_scan_status.txt. The first snapshot is kept whole; each next one narrows the candidates to bytes equal
in every R snapshot, equal in every H snapshot, and different between the two. "report" prints the candidates
(address, released value, held value) and exits. Uses ReadProcessMemory only (no write)."""
import ctypes, ctypes.wintypes as wt, os, sys, time
import numpy as np

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)
TMP = os.environ["TEMP"]
CMD, STATUS = os.path.join(TMP, "ac6_scan_cmd.txt"), os.path.join(TMP, "ac6_scan_status.txt")

class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", wt.DWORD),
                ("PartitionId", wt.WORD), ("RegionSize", ctypes.c_size_t), ("State", wt.DWORD), ("Protect", wt.DWORD),
                ("Type", wt.DWORD)]

def find_pid(name="armoredcore6.exe"):
    arr = (wt.DWORD * 4096)(); got = wt.DWORD()
    psapi.EnumProcesses(arr, ctypes.sizeof(arr), ctypes.byref(got))
    for pid in arr[:got.value // 4]:
        h = k32.OpenProcess(0x1000, False, pid)
        if not h: continue
        buf = ctypes.create_unicode_buffer(512); n = wt.DWORD(512)
        ok = k32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)); k32.CloseHandle(h)
        if ok and buf.value.lower().endswith(name): return pid
    return None

h = k32.OpenProcess(0x0010 | 0x0400, False, find_pid())

def regions():
    out, addr, mbi = [], 0, MBI()
    while k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
        s, z = mbi.BaseAddress or 0, mbi.RegionSize
        if mbi.State == 0x1000 and mbi.Protect == 0x04 and mbi.Type != 0x1000000: out.append((s, z))
        addr = s + z
        if addr >= 0x7FFFFFFFFFFF: break
    return out

def read(addr, n):
    buf = np.empty(n, dtype=np.uint8); got = ctypes.c_size_t()
    if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), buf.ctypes.data_as(ctypes.c_void_p), n, ctypes.byref(got)): return None
    return buf

REGS = regions()
first = None          # label of the first snapshot
full = {}             # region -> whole first snapshot (until the first opposite snapshot)
cand = {}             # region -> (offsets, released values, held values)
n = 0
open(STATUS, "w").write("ready %d regions\n" % len(REGS))
while True:
    if not os.path.exists(CMD): time.sleep(0.1); continue
    cmd = open(CMD).read().split(); os.remove(CMD)
    if not cmd: continue
    if cmd[0] == "report":
        rows = []
        for s, (off, rv, hv) in cand.items():
            for o, r, v in zip(off[:200], rv[:200], hv[:200]): rows.append((s + int(o), int(r), int(v)))
        rows.sort()
        with open(os.path.join(TMP, "ac6_scan_report.txt"), "w") as f:
            f.write("%d candidate byte(s) in %d region(s)\n" % (sum(len(c[0]) for c in cand.values()), len(cand)))
            for a, r, v in rows[:3000]: f.write("%#x R=%02x H=%02x\n" % (a, r, v))
        open(STATUS, "w").write("reported\n")
        break
    label, delay = cmd[0], float(cmd[1]) if len(cmd) > 1 else 0
    time.sleep(delay)
    t0 = time.time()
    n += 1
    if first is None:
        first = label
        for s, z in REGS:
            d = read(s, z)
            if d is not None: full[s] = d
    elif full:
        if label == first: open(STATUS, "w").write("error: need the opposite input second\n"); continue
        for s, z in REGS:
            a = full.pop(s, None)
            if a is None: continue
            b = read(s, z)
            if b is None or len(b) != len(a): continue
            off = np.nonzero(a != b)[0]
            if len(off):
                r, v = (a[off], b[off]) if first == "R" else (b[off], a[off])
                cand[s] = (off, r, v)
        full.clear()
    else:
        for s in list(cand):
            off, rv, hv = cand[s]
            b = read(s, int(off[-1]) + 1)
            if b is None: del cand[s]; continue
            keep = b[off] == (rv if label == "R" else hv)
            if keep.any(): cand[s] = (off[keep], rv[keep], hv[keep])
            else: del cand[s]
    total = sum(len(c[0]) for c in cand.values())
    open(STATUS, "w").write("done %d %s %.1fs candidates %d\n" % (n, label, time.time() - t0, total))
