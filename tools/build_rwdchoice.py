"""build_rwdchoice.py <vanilla 02_gotreward ffdec xml> <out xml>: the reward choice layout (menu/02_rwdchoice.gfx).

From the vanilla "Pièces obtenues" movie (FFDec -swf2xml), on the parts content sprite (218, Dialog/_Content_):
- sprite 900 = a copy of 218 (one part: MakerLogo, ItemIcon/Thumbnail, CategoryName, ItemName + its frame) with the
  button cursor graphic (199) on top, named "Sel" (selection highlight);
- 218 keeps its named children (the game binds them) but loses its decorations, and gets Tile00..Tile11 (sprite 900)
  and "SelIdx" (a copy of the ItemName text field, 901): the DLL writes "selected/count" there;
- Dialog (274) frame 1 decorations get names (Bg0..Bg3, Line0, Line1) so the AS3 can resize the window.
Then: ffdec -xml2swf, and -importScript for MainTimeline (layout + highlight, see rwdchoice_MainTimeline.as)."""
import re, sys

src, dst = sys.argv[1], sys.argv[2]
x = open(src, encoding="utf-8").read()

def sprite_block(sid):
    m = re.search(r'<item type="DefineSpriteTag"[^>]*\bspriteId="%s">.*?</subTags>\s*</item>' % sid, x, re.S)
    assert m, sid
    return m

def place(char, depth, name, sx=None, tx=0, ty=0):
    scale = ('hasScale="true" nScaleBits="20" scaleX="%s" scaleY="%s" ' % (sx, sx)) if sx else 'hasScale="false" nScaleBits="0" '
    return ('<item type="PlaceObject2Tag" characterId="%d" depth="%d" forceWriteAsLong="true" name="%s" '
            'placeFlagHasCharacter="true" placeFlagHasClipActions="false" placeFlagHasClipDepth="false" '
            'placeFlagHasColorTransform="false" placeFlagHasMatrix="true" placeFlagHasName="true" '
            'placeFlagHasRatio="false" placeFlagMove="false">\n'
            '<matrix type="MATRIX" hasRotate="false" %snRotateBits="0" nTranslateBits="17" translateX="%d" translateY="%d"/>\n'
            '</item>\n') % (char, depth, name, scale, tx, ty)

# --- tile sprite 900 = copy of 218 + "Sel" highlight
m218 = sprite_block(218)
tile = m218.group().replace('spriteId="218"', 'spriteId="900"', 1)
tile = tile.replace('<item type="ShowFrameTag"', place(199, 40, "Sel") + '<item type="ShowFrameTag"', 1)

# --- SelIdx text field 901 = copy of the ItemName field 204
m204 = re.search(r'<item type="DefineEditTextTag"[^>]*\bcharacterID="204".*?</item>', x, re.S)
seltext = m204.group().replace('characterID="204"', 'characterID="901"', 1).replace('initialText="HD-XXXXXXXXXXXXX"', 'initialText="-"', 1)

# --- 218: drop unnamed placements (decorations), add the tiles and SelIdx
body = m218.group()
body = re.sub(r'<item type="PlaceObject\d?Tag"(?![^>]*\bname=)[^>]*>.*?</item>\n?', '', body, flags=re.S)
adds = "".join(place(900, 30 + i, "Tile%02d" % i) for i in range(12)) + place(901, 60, "SelIdx")
body = body.replace('<item type="ShowFrameTag"', adds + '<item type="ShowFrameTag"', 1)

x = x[:m218.start()] + tile + "\n" + seltext + "\n" + body + x[m218.end():]

# --- Dialog 274: name the frame-1 decorations
m274 = sprite_block(274)
d = m274.group()
first_frame = d.find('<item type="ShowFrameTag"')
head, rest = d[:first_frame], d[first_frame:]
for depth, name in ((1, "Bg0"), (3, "Bg1"), (4, "Bg2"), (8, "Bg3"), (72, "Line0"), (74, "Line1")):
    pat = r'(<item type="PlaceObject2Tag"[^>]*\bdepth="%d"[^>]*?)placeFlagHasName="false"' % depth
    head, n = re.subn(pat, r'\1name="%s" placeFlagHasName="true"' % name, head, count=1)
    assert n == 1, (depth, name)
x = x[:m274.start()] + head + rest + x[m274.end():]

open(dst, "w", encoding="utf-8").write(x)
print("ok:", dst)
