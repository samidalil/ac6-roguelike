"""ac6_lib: read armoredcore6.exe (1.09) memory from Python without Cheat Engine.
Needs: pip install capstone. Non-elevated same-user process => ReadProcessMemory works.
Provides: img (module image bytes), rpm(addr,n), q(addr), callers(target), dis(start,end), func_info(start).
"""
import ctypes, re, struct, capstone
K = ctypes.windll.kernel32
import subprocess, os
def _find_pid():
    if os.environ.get("AC6_PID"): return int(os.environ["AC6_PID"])
    out = subprocess.check_output(["tasklist", "/FI", "IMAGENAME eq armoredcore6.exe", "/FO", "CSV", "/NH"], text=True)
    m = re.search(r'"armoredcore6.exe","(\d+)"', out)
    if not m: raise SystemExit("armoredcore6.exe not running (or set AC6_PID)")
    return int(m.group(1))
PID, BASE, SIZE = _find_pid(), 0x140000000, 0x6C2FC00   # 1.09, no ASLR observed
h = K.OpenProcess(0x0410, False, PID)
def rpm(addr, n):
    buf = ctypes.create_string_buffer(n); rd = ctypes.c_size_t()
    return buf.raw[:rd.value] if K.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(rd)) else b""
img = b"".join((lambda b: b + b"\0"*(0x10000-len(b)))(rpm(BASE+o, 0x10000)) for o in range(0, SIZE, 0x10000))
def q(a):
    b = rpm(a, 8); return struct.unpack("<Q", b)[0] if len(b) == 8 else None
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64); md.detail = True
X86 = capstone.x86
def callers(target):
    out = []
    for m in re.finditer(rb"[\xe8\xe9]", img):
        s = m.start()
        if s + 5 <= len(img) and s + 5 + struct.unpack("<i", img[s+1:s+5])[0] == target: out.append((s, img[s]))
    return out
def annot(i):
    for op in i.operands:
        if op.type == X86.X86_OP_MEM and op.mem.base == X86.X86_REG_RIP:
            t = i.address + i.size + op.mem.disp - BASE; v = q(BASE + t)
            return "  ; exe+%X = %s" % (t, ("%X" % v) if v is not None else "?")
        if op.type == X86.X86_OP_IMM and i.mnemonic in ("call", "jmp") and BASE <= op.imm < BASE + SIZE:
            return "  ; -> exe+%X" % (op.imm - BASE)
    return ""
def dis(start, end, mark=None):
    for i in md.disasm(img[start:end], BASE + start):
        print("   %sexe+%-8X %-7s %s%s" % (">>" if i.address - BASE == mark else "  ", i.address - BASE, i.mnemonic, i.op_str, annot(i)))
def func_info(start, maxlen=0x1000):
    """linear sweep from start until ret past 0x40 bytes or maxlen: direct call targets + rip-rel loads"""
    calls, loads = [], []
    for i in md.disasm(img[start:start+maxlen], BASE + start):
        if i.mnemonic == "call" and i.operands and i.operands[0].type == X86.X86_OP_IMM: calls.append(i.operands[0].imm - BASE)
        for op in i.operands:
            if op.type == X86.X86_OP_MEM and op.mem.base == X86.X86_REG_RIP and i.mnemonic in ("mov", "lea"):
                loads.append(i.address + i.size + op.mem.disp - BASE)
        if i.mnemonic in ("ret",) and i.address - BASE - start > 0x40: break
        if i.mnemonic == "int3": break
    return calls, loads
