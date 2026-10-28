"""dis_at.py <rva> [<rva>...] [--after N]: disassemble the function containing each rva (start = after the previous
int3 padding), up to N bytes past the rva (default 0x80), marking the rva."""
import sys
from ac6_lib import img, dis

def func_start(a, back=0x3000):
    for s in range(a, max(0, a - back), -1):
        if img[s - 1] == 0xCC and img[s - 2] == 0xCC: return s
    return a - 0x40

args = [x for x in sys.argv[1:] if not x.startswith("--")]
after = int(sys.argv[sys.argv.index("--after") + 1], 0) if "--after" in sys.argv else 0x80
args = [x for x in args if x != str(after) and x != hex(after)]
for x in args:
    a = int(x, 16)
    s = func_start(a)
    print("=== exe+%X (function exe+%X)" % (a, s))
    dis(s, a + after, mark=a)
