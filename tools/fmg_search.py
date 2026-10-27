"""fmg_search.py <text>: find message ids whose text contains <text> (case-insensitive), in every version and
category of the live MsgRepository."""
import sys, struct
from fmg import REPO, u32
from ac6_lib import q, rpm

def texts(f):
    ng, offs = u32(f + 0xC), q(f + 0x18)
    for g in range(ng):
        si, first, last = struct.unpack("<iii", rpm(f + 0x28 + g * 16, 12))
        for k in range(last - first + 1):
            o = q(offs + (si + k) * 8)
            if not o: continue
            raw = rpm(f + o, 600)
            n = next((i for i in range(0, len(raw) - 1, 2) if raw[i:i + 2] == b"\0\0"), len(raw))
            yield first + k, raw[:n].decode("utf-16-le", "replace")

needle = sys.argv[1].lower()
nver, ncat = u32(REPO + 0x10), u32(REPO + 0x14)
for v in range(nver):
    pv = q(q(REPO + 8) + v * 8)
    if not pv: continue
    for c in range(ncat):
        f = q(pv + c * 8)
        if not f: continue
        for id_, t in texts(f):
            if needle in t.lower(): print(f"version {v} cat {c} id {id_}: {t[:120]!r}")
