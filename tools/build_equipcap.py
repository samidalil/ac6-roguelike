"""build_equipcap.py <vanilla 02_010_equiptop ffdec xml> <out xml>: the inventory cap's sell list (menu/02_010_equipcap.gfx).

From the vanilla part list movie (FFDec -swf2xml), on the main timeline: every tag at the depths of the 3D AC view
(PlayerModelView, sprite 228, depth 38) and of the AC specs slot (DetailStatusView, sprite 270, depth 90: the game puts
its 02_050_DetailStatus_Player window on DetailStatusView/AdjustPosTop/Status_Player) is removed, the placement and
its animation moves. The rest (category tabs, title, key guide) is untouched: the game binds those names. The list,
part info and part stats are other movies (02_Equip, 02_GaitemStatus_*: full-screen windows at fixed positions).
Then: ffdec -xml2swf, and -importScript for MainTimeline (see equipcap_MainTimeline.as)."""
import re, sys

src, dst = sys.argv[1], sys.argv[2]
x = open(src, encoding="utf-8").read()

REMOVED = {"38": "PlayerModelView", "90": "DetailStatusView"}
sprites = [(m.start(), m.end()) for m in re.finditer(r'<item type="DefineSpriteTag".*?</subTags>\s*</item>', x, re.S)]
def top(p): return not any(a <= p < b for a, b in sprites)

tags = [m for m in re.finditer(r'<item type="(?:PlaceObject\d?Tag|RemoveObject\d?Tag)"[^>]*\bdepth="(\d+)"[^>]*?(?:/>|>.*?</item>)\n?', x, re.S)
        if m.group(1) in REMOVED and top(m.start())]
names = {d: sum(1 for m in tags if m.group(1) == d) for d in REMOVED}
for d, n in REMOVED.items():
    assert re.search(r'\bdepth="%s"[^>]*\bname="%s"' % (d, n), "".join(m.group() for m in tags)), (d, n)
for m in reversed(tags): x = x[:m.start()] + x[m.end():]

open(dst, "w", encoding="utf-8").write(x)
print("removed: " + ", ".join("%s (depth %s, %d tags)" % (n, d, names[d]) for d, n in REMOVED.items()))
