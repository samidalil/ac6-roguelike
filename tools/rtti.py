"""rtti.py <vtable rva|abs addr>...  — print the MSVC RTTI class name of a vtable (via the complete object locator)."""
import sys, struct
from ac6_lib import BASE, q, rpm
def rtti_name(vt):
    col = q(vt - 8)
    if not col: return None
    d = rpm(col, 0x18)
    if len(d) < 0x18: return None
    td_rva = struct.unpack("<I", d[0xC:0x10])[0]
    name = rpm(BASE + td_rva + 0x10, 200).split(b"\0")[0]
    return name.decode("ascii", "replace")
if __name__ == "__main__":
    for a in sys.argv[1:]:
        v = int(a, 16); v = v if v > BASE else BASE + v
        print("%X  %s" % (v - BASE, rtti_name(v)))
