"""paramdef.py <ParamName> [field substring...] — field offsets of a param row from its Smithbox definition
(Assets/PARAM/AC6/Defs/<ParamName>.xml): handles scalar types, fixstr/fixstrW/dummy8 arrays and bitfields."""
import re, sys
DEFS = r"D:\samee\Workspace\Modding\PC\From Software\Smithbox_2_2_5_2026_08_29_a\Assets\PARAM\AC6\Defs"
SIZE = {"s8": 1, "u8": 1, "s16": 2, "u16": 2, "s32": 4, "u32": 4, "f32": 4, "angle32": 4, "f64": 8, "b32": 4,
        "fixstr": 1, "fixstrW": 2, "dummy8": 1}

def layout(name):
    xml = open(f"{DEFS}\\{name}.xml", encoding="utf-8-sig").read()
    fields = re.findall(r'<Field Def="([^"]+)"', xml)
    out, off, bit_type, bit_pos = [], 0, None, 0
    for f in fields:
        m = re.match(r"(\w+)\s+(\w+)(?:\[(\d+)\])?(?::(\d+))?", f)
        typ, fname, count, bits = m.group(1), m.group(2), m.group(3), m.group(4)
        size = SIZE[typ]
        if bits is not None:
            bits = int(bits)
            # continue the current bit run if same storage type and it fits
            if bit_type == size and bit_pos + bits <= size * 8:   # runs continue across u8/dummy8 of the same width
                out.append((off - size, fname, typ, f"bits {bit_pos}..{bit_pos + bits - 1}"))
                bit_pos += bits
                continue
            bit_type, bit_pos = size, bits
            out.append((off, fname, typ, f"bits 0..{bits - 1}"))
            off += size
            continue
        bit_type, bit_pos = None, 0
        n = int(count) if count else 1
        out.append((off, fname, typ, f"x{n}" if count else ""))
        off += size * n
    return out, off

if __name__ == "__main__":
    rows, total = layout(sys.argv[1])
    subs = [s.lower() for s in sys.argv[2:]]
    for off, fname, typ, note in rows:
        if not subs or any(s in fname.lower() for s in subs):
            print(f"+{off:#06x} {typ:<8} {fname} {note}")
    print(f"row size {total:#x}")
