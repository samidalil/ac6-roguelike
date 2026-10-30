"""build_capmovies.py <ffdec xml> <out xml> <dx px> [<kept name>...]: a movie of the inventory cap's sell screen moved
right by dx pixels (the screen without the AC specs is centred).

Every placement and move on the main timeline (PlaceObject tags outside the sprites) gets translateX + dx * 20 twips,
except the depths of the kept instance names (StatusBar, the key guide, and AdditionalProperties, the credits box, stay
at their vanilla place). A new placement without a matrix gets one. Used for the cap's copies of the list window (02_010_EquipTop, after
build_equipcap.py), the list (02_Equip) and the part info / stats (02_GaitemStatus_General_*): full-screen windows the
game draws at fixed positions. Then: ffdec -xml2swf."""
import re, sys

src, dst, dx = sys.argv[1], sys.argv[2], int(sys.argv[3]) * 20
kept_names = set(sys.argv[4:])
x = open(src, encoding="utf-8").read()

sprites = [(m.start(), m.end()) for m in re.finditer(r'<item type="DefineSpriteTag".*?</subTags>\s*</item>', x, re.S)]
def top(p): return not any(a <= p < b for a, b in sprites)

PLACE = re.compile(r'<item type="PlaceObject\d?Tag"([^>]*)>(.*?)</item>', re.S)
places = [m for m in PLACE.finditer(x) if top(m.start())]
kept = {re.search(r'\bdepth="(\d+)"', m.group(1)).group(1) for m in places
        if (n := re.search(r'\bname="([^"]*)"', m.group(1))) and n.group(1) in kept_names}
assert len(kept) == len(kept_names), ("kept names not found", kept_names, kept)

def bits(*v):
    return max([0] + [(abs(a) if a >= 0 else abs(a) - 1).bit_length() + 1 for a in v if a])

def shift(m):
    attrs, body = m.group(1), m.group(2)
    if re.search(r'\bdepth="(\d+)"', attrs).group(1) in kept: return m.group(0)
    if 'placeFlagHasMatrix="false"' in attrs:
        if 'placeFlagMove="true"' in attrs: return m.group(0)  # a move keeping the shifted matrix
        attrs = attrs.replace('placeFlagHasMatrix="false"', 'placeFlagHasMatrix="true"')
        body = '<matrix type="MATRIX" hasRotate="false" hasScale="false" nRotateBits="0" nScaleBits="0" nTranslateBits="0" translateX="0" translateY="0"/>' + body
    mx = re.search(r'<matrix[^>]*/>', body)
    t = mx.group(0)
    tx = int(re.search(r'translateX="(-?\d+)"', t).group(1)) + dx
    ty = int(re.search(r'translateY="(-?\d+)"', t).group(1))
    t = re.sub(r'translateX="-?\d+"', 'translateX="%d"' % tx, t)
    t = re.sub(r'nTranslateBits="\d+"', 'nTranslateBits="%d"' % bits(tx, ty), t)
    return '<item type="%s"%s>%s</item>' % (re.match(r'<item type="([^"]+)"', m.group(0)).group(1), attrs,
                                            body[:mx.start()] + t + body[mx.end():])

n = 0
for m in reversed(places):
    s = shift(m)
    if s != m.group(0): n += 1
    x = x[:m.start()] + s + x[m.end():]
open(dst, "w", encoding="utf-8").write(x)
print("%s: %d of %d main-timeline placements moved %+d px%s" % (dst, n, len(places), dx // 20,
      ", kept depth " + ", ".join(sorted(kept)) if kept else ""))
