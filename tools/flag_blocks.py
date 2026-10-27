"""CE-free survey of CSEventFlagMan's block map: every block (= flag id // 1000) the game allocated, its storage
type (1 = page in the shared page array, 2 = separately allocated page), and which of its flags are set.
Used to pick flag ranges for the mod's save state and to prove they persist (set, save, relaunch, read back).
Usage:
  python flag_blocks.py                      list blocks: id, type, set-bit count
  python flag_blocks.py --show 4 7           list the set flag ids of blocks 4 and 7
  python flag_blocks.py --snap FILE          write {block: hex page} of every block to FILE (json)
  python flag_blocks.py --diff FILE          flags that changed since the snapshot FILE
  python flag_blocks.py --set ID VAL [ID VAL ...]   write flags (direct bit write + dirty byte, like the DLL)
  python flag_blocks.py --save               request a save (bit 0 of the save request mask, like the DLL)"""
import ctypes, json, struct, sys
from ac6_lib import PID, BASE
K = ctypes.windll.kernel32
h = K.OpenProcess(0x0438, False, PID); assert h, "OpenProcess (read/write) failed"
def rpm(a, n):
    b = ctypes.create_string_buffer(n); r = ctypes.c_size_t()
    return b.raw[:r.value] if K.ReadProcessMemory(h, ctypes.c_void_p(a), b, n, ctypes.byref(r)) else b""
def wpm(a, data):
    r = ctypes.c_size_t(); return bool(K.WriteProcessMemory(h, ctypes.c_void_p(a), data, len(data), ctypes.byref(r)))
q = lambda a: struct.unpack("<Q", rpm(a, 8).ljust(8, b"\0"))[0]
u32 = lambda a: struct.unpack("<I", rpm(a, 4).ljust(4, b"\0"))[0]
man = q(BASE + 0x4E13070); div = u32(man + 0x1C); bsz = u32(man + 0x20); bbase = q(man + 0x28)
assert man and div, "CSEventFlagMan not built yet"

def blocks():
    """In-order walk of the block rb-tree: {block: (type, page address)}."""
    hdr = q(man + 0x38); out = {}
    def walk(n, depth=0):
        if not n or rpm(n + 0x19, 1) == b"\x01" or depth > 64: return
        walk(q(n), depth + 1)
        blk, typ, v = u32(n + 0x20), u32(n + 0x28), q(n + 0x30)
        out[blk] = (typ, bbase + bsz * (v & 0xFFFFFFFF) if typ == 1 else v if typ == 2 else 0)
        walk(q(n + 0x10), depth + 1)
    walk(q(hdr + 8))
    return out
def set_ids(blk, page):
    data = rpm(page, bsz)
    return [blk * div + i * 8 + b for i, x in enumerate(data) for b in range(8) if x & (0x80 >> b)]
def bitaddr(fid):
    t, p = blocks().get(fid // div, (0, 0)); sub = fid % div
    return (p + (sub >> 3), 0x80 >> (sub & 7)) if p else (None, None)

bl = blocks()
a = sys.argv[1:]
if not a:
    print("man=%X div=%d bytes/block=%d pages=%X, %d blocks" % (man, div, bsz, bbase, len(bl)))
    for blk, (t, p) in bl.items():
        print("block %7d (flags %d..%d) type %d page %X: %d set" % (blk, blk * div, blk * div + div - 1, t, p, len(set_ids(blk, p)) if p else -1))
elif a[0] == "--show":
    for blk in map(int, a[1:]):
        t, p = bl.get(blk, (0, 0)); print("block %d type %d:" % (blk, t), set_ids(blk, p) if p else "absent")
elif a[0] == "--snap":
    json.dump({str(b): rpm(p, bsz).hex() for b, (t, p) in bl.items() if p}, open(a[1], "w")); print("snapshot of %d blocks -> %s" % (len(bl), a[1]))
elif a[0] == "--diff":
    old = json.load(open(a[1]))
    for blk, (t, p) in bl.items():
        o = bytes.fromhex(old.get(str(blk), "")) if str(blk) in old else None
        if o is None: print("block %d: NEW since the snapshot" % blk); continue
        cur = rpm(p, bsz)
        for i in range(min(len(o), len(cur))):
            d = o[i] ^ cur[i]
            for b in range(8):
                if d & (0x80 >> b): print("flag %d: %d -> %d" % (blk * div + i * 8 + b, bool(o[i] & (0x80 >> b)), bool(cur[i] & (0x80 >> b))))
    for blk in old:
        if int(blk) not in bl: print("block %s: GONE since the snapshot" % blk)
elif a[0] == "--set":
    for fid, val in zip(map(int, a[1::2]), map(int, a[2::2])):
        ad, m = bitaddr(fid)
        if ad is None: print("flag %d: block absent" % fid); continue
        x = rpm(ad, 1)[0]; wpm(ad, bytes([(x | m) if val else (x & ~m & 0xFF)])); wpm(man + 0x60, b"\x01")
        print("flag %d: %d -> %d" % (fid, bool(x & m), bool(rpm(ad, 1)[0] & m)))
elif a[0] == "--save":
    st = q(BASE + 0x4E140B0); mask = u32(st + 0xCB4); wpm(st + 0xCB4, struct.pack("<I", mask | 1)); print("save requested (mask was %#x)" % mask)
