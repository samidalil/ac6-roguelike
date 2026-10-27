"""gfx_xml_info.py <ffdec swf2xml file> <sprite id>...: the PlaceObject tags (character, depth, name, matrix) of the
given DefineSprite tags, to plan layout edits done on the XML (then ffdec -xml2swf)."""
import re, sys

x = open(sys.argv[1], encoding="utf-8").read()

def attr(s, k):
    m = re.search(r'\b' + k + r'="([^"]*)"', s)
    return m.group(1) if m else ""

for sid in sys.argv[2:]:
    m = re.search(r'<item type="DefineSpriteTag"[^>]*\bspriteId="%s".*?</subTags>' % sid, x, re.S)
    if not m: print("no sprite", sid); continue
    s = m.group()
    print("=== sprite %s (frames %s)" % (sid, attr(s[:300], "frameCount")))
    for t in re.finditer(r'<item type="(PlaceObject\d?Tag|ShowFrameTag|RemoveObject\d?Tag)"([^>]*)>(.*?)(?=<item type=|</subTags>)', s, re.S):
        if t.group(1) == "ShowFrameTag": print("   -- frame"); continue
        a, body = t.group(2), t.group(3)
        mx = re.search(r'<matrix[^>]*>', body)
        mt = ""
        if mx:
            m0 = mx.group()
            mt = "tx %s ty %s sx %s sy %s" % (attr(m0, "translateX"), attr(m0, "translateY"), attr(m0, "scaleX"), attr(m0, "scaleY"))
        print("   %s char %s depth %s name %s %s" % (t.group(1), attr(a, "characterId"), attr(a, "depth"), attr(a, "name"), mt))
