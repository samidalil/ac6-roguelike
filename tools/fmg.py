"""fmg.py — read the game's live message tables (MsgRepository [exe+4E256D8], version 0).
  python fmg.py find <id> [<id>...]   -> every category holding that id, with its text
  python fmg.py get <category> <id>
"""
import sys, struct
from ac6_lib import BASE, q, rpm
REPO = q(BASE + 0x4E256D8)
def u32(a): return struct.unpack("<I", rpm(a, 4))[0]
def fmg_of(cat):
    nver, ncat = u32(REPO + 0x10), u32(REPO + 0x14)
    if cat >= ncat: return 0
    pv = q(q(REPO + 8)); return q(pv + cat * 8) if pv else 0
def get(fmg, id_):
    if not fmg: return None
    ngroups = u32(fmg + 0xC); offs = q(fmg + 0x18)
    for g in range(ngroups):
        si, first, last = struct.unpack("<iii", rpm(fmg + 0x28 + g * 16, 12))
        if first <= id_ <= last:
            o = q(offs + (si + id_ - first) * 8)
            if not o: return None
            raw = rpm(fmg + o, 1024)
            n = next((i for i in range(0, len(raw) - 1, 2) if raw[i:i+2] == b"\0\0"), len(raw))
            return raw[:n].decode("utf-16-le", "replace")
    return None
if __name__ == "__main__":
    if sys.argv[1] == "find":
        ncat = u32(REPO + 0x14)
        for id_ in map(int, sys.argv[2:]):
            for c in range(ncat):
                t = get(fmg_of(c), id_)
                if t: print(f"{id_}: cat {c}: {t[:80]!r}")
    else:
        print(get(fmg_of(int(sys.argv[2])), int(sys.argv[3])))
