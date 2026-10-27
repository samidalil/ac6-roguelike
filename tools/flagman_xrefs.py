from ac6_lib import *
G = 0x4E13070
inst = q(BASE + G); print("CSEventFlagMan instance = %X" % inst)
vt = q(inst); print("vtable = exe+%X" % (vt - BASE))
vfs = [q(vt + 8*i) for i in range(40)]
print("vtable entries containing AD6B60: %s" % [i for i, v in enumerate(vfs) if v == BASE + 0xAD6B60])
print("object layout (0x1C0 bytes):")
for off in range(0, 0x1C0, 8):
    v = q(inst + off)
    t = ("exe+%X" % (v - BASE)) if v and BASE <= v < BASE + SIZE else ("heap" if v and 0x7FF000000000 <= v < 0x7FFF00000000 else "")
    if v: print("   +%03X %016X %s" % (off, v, t))
# loaders of the global
sites = []
for m in re.finditer(rb"[\x48\x4c](?:\x8d|\x8b)[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]", img):
    s = m.start()
    if s + 7 + struct.unpack("<i", img[s+3:s+7])[0] == G: sites.append(s)
print("\nloaders of exe+4E13070: %d sites" % len(sites)); print(["exe+%X" % s for s in sites[:60]])
# builder call graph BFS
roots = [0xE88F00, 0xE881D0, 0xE89B80, 0x19B7540, 0xE84650, 0xE8CB90]
seen, frontier, depth = set(roots), list(roots), 0
edges = {}
while frontier and depth < 4:
    nxt = []
    for f in frontier:
        calls, loads = func_info(f, 0x1200)
        edges[f] = sorted(set(calls))
        for c in set(calls):
            if c not in seen and BASE <= BASE + c < BASE + SIZE: seen.add(c); nxt.append(c)
    frontier = nxt; depth += 1
print("\nbuilder reachable functions (depth<=4): %d" % len(seen))
hits = []
for f in sorted(seen):
    for s in sites:
        if f <= s < f + 0x800: hits.append((f, s))
print("reachable functions that load CSEventFlagMan: %s" % ["func exe+%X site exe+%X" % (f, s) for f, s in hits])
for f, s in hits[:6]:
    # path from root
    path = [f]
    while True:
        p = next((k for k, v in edges.items() if path[0] in v), None)
        if p is None or p in path: break
        path.insert(0, p)
    print("\n== func exe+%X (path %s) ==" % (f, " -> ".join("exe+%X" % x for x in path)))
    dis(s - 0x30, s + 0x60, s)
