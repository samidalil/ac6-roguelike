"""live_param.py [--list] | <ParamName> [row ...] [--fields substr...]: rows of a live param from the game's param
repository [exe+4E27300] (the DLL's param_index/param_file), decoded with the Smithbox definition (paramdef.py)."""
import struct, sys
from ac6_lib import rpm, q, BASE
import paramdef

def u32(a): return struct.unpack("<I", rpm(a, 4))[0]
def u16(a): return struct.unpack("<H", rpm(a, 2))[0]

def params():
    repo = q(BASE + 0x4E27300)
    out = {}
    for i in range(0x109):
        slot = repo + 0x80 + i * 0x48
        res = q(slot + 8)
        if not u32(slot) or not res: continue
        ln, cap = q(res + 0x18 + 0x10), q(res + 0x18 + 0x18)
        s = q(res + 0x18) if cap >= 8 else res + 0x18
        name = rpm(s, ln * 2).decode("utf-16-le", "replace")
        f = q(q(res + 0x80) + 0x80)
        out[name] = (i, f)
    return out

def rows(f):
    n = u16(f + 0x0A)
    return [(u32(f + 0x40 + i * 0x18), f + q(f + 0x40 + i * 0x18 + 8)) for i in range(n)]

FMT = {"s8": "b", "u8": "B", "s16": "h", "u16": "H", "s32": "i", "u32": "I", "f32": "f", "angle32": "f", "b32": "i"}

if __name__ == "__main__":
    ps = params()
    if sys.argv[1] == "--list":
        for k, (i, f) in sorted(ps.items(), key=lambda x: x[1][0]): print(i, k, u16(f + 0x0A) if f else "-")
        sys.exit()
    name = sys.argv[1]
    args = sys.argv[2:]
    defn = name
    if "--def" in args: k = args.index("--def"); defn = args[k + 1]; args = args[:k] + args[k + 2:]
    subs = args[args.index("--fields") + 1:] if "--fields" in args else []
    want = [int(a) for a in (args[:args.index("--fields")] if "--fields" in args else args)]
    lay, size = paramdef.layout(defn)
    i, f = ps[name]
    print(f"{name}: repo index {i}, {u16(f + 0x0A)} rows, row size {size:#x}")
    for rid, r in rows(f):
        if want and rid not in want: continue
        data = rpm(r, size)
        vals = []
        for off, fn, typ, note in lay:
            if typ in ("dummy8", "fixstr", "fixstrW") or typ not in FMT: continue
            if subs and not any(s.lower() in fn.lower() for s in subs): continue
            if note.startswith("bits"):
                lo, hi = map(int, note[5:].split(".."))
                v = (struct.unpack_from("<" + FMT[typ], data, off)[0] >> lo) & ((1 << (hi - lo + 1)) - 1)
            else: v = struct.unpack_from("<" + FMT[typ], data, off)[0]
            if v != 0 or subs: vals.append(f"{fn}={round(v, 4) if isinstance(v, float) else v}")
        print(rid, " ".join(vals))
