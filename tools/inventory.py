"""inventory.py — every owned part: name, equipped?, lineup price (None = the DLL cannot sell it)."""
import stash as S
rows = {}
import struct
for i in range(S.nrows):
    off = S.q(S.f + 0x40 + i * 0x18 + 8); r = S.f + off
    eid, p, typ, rew = S.u32(r), S.s32(r + 4), S.rpm(r + 0xC, 1)[0], S.s32(r + 0x10)
    rows.setdefault(eid & 0x0FFFFFFF, []).append((i, p, typ, rew))
for i in range(min(S.u32(S.mgr + 8), 4096)):
    e = S.entry(i); h, item = S.u32(e), S.u32(e + 4)
    if h == 0xFFFFFFFF or item >> 28 not in (0, 1, 5, 6, 7): continue
    print("%-3d %08X %-26s %-9s price=%-8s lineup=%s" % (i, item, S.name(item), "EQUIPPED" if h in S.equipped else "", S.price.get(item), rows.get(item & 0x0FFFFFFF)))
