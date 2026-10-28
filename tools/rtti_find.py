"""rtti_find.py <regex>: vtables (rva) of every RTTI class whose decorated name matches <regex>, with slot 0..5."""
import re, struct, sys
from ac6_lib import img, BASE, SIZE

pat = re.compile(sys.argv[1].encode())
# type descriptors: {vftable ptr, spare, name ".?AV..."}; name at td+0x10
tds = {}
for m in re.finditer(rb"\.\?A[VU][^\x00]{3,400}\x00", img):
    if pat.search(m.group()): tds[m.start() - 0x10] = m.group()[:-1].decode("ascii", "replace")
if not tds: sys.exit("no type descriptor matches")
# complete object locators: {sig=1, offset, cdOffset, td rva, chd rva, self rva}
cols = {}
for off in range(0, len(img) - 0x18, 4):
    if img[off:off + 4] != b"\x01\x00\x00\x00": continue
    td = struct.unpack_from("<I", img, off + 0xC)[0]
    if td in tds and struct.unpack_from("<I", img, off + 0x14)[0] == off: cols[off] = td
# vtables: a pointer to the COL right before slot 0
colptr = {BASE + c: c for c in cols}
for off in range(0, len(img) - 8, 8):
    v = struct.unpack_from("<Q", img, off)[0]
    if v in colptr:
        vt = off + 8
        slots = [struct.unpack_from("<Q", img, vt + 8 * k)[0] - BASE for k in range(6)]
        print("vtable exe+%X  %s\n   slots %s" % (vt, tds[cols[colptr[v]]], " ".join("%X" % s for s in slots)))
