"""snap.py — snapshot / diff the memory of a few game singletons, to find a state flag by comparing game states.
  python snap.py save <name>            dump the objects into snaps/<name>.json
  python snap.py diff <a> <b> [<c>...]  offsets whose bytes are equal within each group and differ between groups:
                                        groups are separated by "vs" (e.g. diff ingame garage vs title_proper)
"""
import json, os, sys
from ac6_lib import BASE, q, rpm
HERE = os.path.dirname(os.path.abspath(__file__))
SNAPS = os.path.join(HERE, "snaps")
# (name, global RVA, bytes to dump)
OBJECTS = [("save_state", 0x4E140B0, 0x2000), ("game_man", 0x4E29CC0, 0x6000), ("menu_man", 0x4E15530, 0x1000),
           ("game_data_man", 0x4E03A08, 0x200)]

def save(name):
    os.makedirs(SNAPS, exist_ok=True)
    out = {}
    for n, rva, size in OBJECTS:
        p = q(BASE + rva)
        out[n] = {"addr": p, "hex": rpm(p, size).hex() if p else ""}
        print(f"{n}: {p:#x} ({len(out[n]['hex']) // 2} bytes)" if p else f"{n}: null")
    json.dump(out, open(os.path.join(SNAPS, name + ".json"), "w"))

def diff(groups):
    data = [[json.load(open(os.path.join(SNAPS, n + ".json"))) for n in g] for g in groups]
    for obj, _, _ in OBJECTS:
        blobs = [[bytes.fromhex(s[obj]["hex"]) for s in g] for g in data]
        n = min(len(b) for g in blobs for b in g)
        for off in range(n):
            vals = [{b[off] for b in g} for g in blobs]
            if all(len(v) == 1 for v in vals) and len({next(iter(v)) for v in vals}) == len(vals):
                print(f"{obj}+{off:#x}: " + " | ".join(f"{next(iter(v)):02x}" for v in vals))

if __name__ == "__main__":
    if sys.argv[1] == "save": save(sys.argv[2])
    else:
        groups, cur = [], []
        for a in sys.argv[2:]:
            if a == "vs": groups.append(cur); cur = []
            else: cur.append(a)
        groups.append(cur)
        diff(groups)
