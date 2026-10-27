import ctypes, ctypes.wintypes as w, re, struct, sys
K = ctypes.windll.kernel32
from ac6_lib import PID, BASE, SIZE
h = K.OpenProcess(0x0410, False, PID)
assert h, "OpenProcess failed"
def rpm(addr, n):
    buf = ctypes.create_string_buffer(n); rd = ctypes.c_size_t()
    ok = K.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(rd))
    return buf.raw[:rd.value] if ok else b""
# read image in 64K chunks (skip unreadable)
CH = 0x10000
img = bytearray()
for off in range(0, SIZE, CH):
    b = rpm(BASE + off, CH)
    if len(b) < CH: b = b + b"\0" * (CH - len(b))
    img += b
img = bytes(img)
print("image read: %d bytes" % len(img))
def q(addr):
    b = rpm(addr, 8); return struct.unpack("<Q", b)[0] if len(b) == 8 else None
names = sys.argv[1:] or ["CSEventFlagMan", "GameDataMan"]
# rip-relative lea/mov: 48/4C 8D modrm(…101) disp32 ; 48/4C 8B modrm disp32
LEA = re.compile(rb"[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]")
MOV = re.compile(rb"[\x48\x4c]\x8b[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]")
leas = [(m.start(), m.start() + 7 + struct.unpack("<i", img[m.start()+3:m.start()+7])[0]) for m in LEA.finditer(img)]
print("lea rip-rel sites: %d" % len(leas))
bytarget = {}
for site, tgt in leas: bytarget.setdefault(tgt, []).append(site)
for name in names:
    for enc, label in ((name.encode("utf-16-le"), "utf16"), (name.encode("ascii"), "ascii")):
        for m in re.finditer(re.escape(enc) + rb"\x00", img):
            off = m.start()
            # require string start (previous byte 0) to avoid suffix matches
            if img[off-1] != 0: continue
            saddr = BASE + off
            sites = bytarget.get(off, [])
            print("\n[%s %s] string @ exe+%X  (%d lea xrefs)" % (name, label, off, len(sites)))
            for s in sites[:12]:
                print("   lea site exe+%X" % s)
                # nearby rip-relative movs within -0x60..+0x60 -> candidate globals
                lo, hi = max(0, s - 0x60), s + 0x60
                for mm in MOV.finditer(img, lo, hi):
                    ms = mm.start(); disp = struct.unpack("<i", img[ms+3:ms+7])[0]; g = ms + 7 + disp
                    if 0 <= g < SIZE:
                        v = q(BASE + g)
                        print("      mov rip-rel @exe+%X -> global exe+%X  [global]=%s" % (ms, g, ("%X" % v) if v is not None else "?"))
