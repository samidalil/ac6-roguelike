"""stash.py — the player's sellable stash (owned, not equipped, priced) and the equipped starters, like the DLL's
failure plan sees them. Prices from the live EquipmentLineupParam (param repository [exe+4E27300], slot 0x29)."""
import struct, re
from ac6_lib import BASE, q, rpm
from fmg import get, fmg_of
u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]
s32 = lambda a: struct.unpack("<i", rpm(a, 4))[0]
TAG = {0: 0, 1: 0x10000000, 4: 0x60000000, 5: 0x70000000, 6: 0x50000000}
NAMECAT = {0: 11, 1: 12, 6: 38, 5: 35, 7: 41}
repo = q(BASE + 0x4E27300); res = q(repo + 0x80 + 0x29 * 0x48 + 8); f = q(q(res + 0x80) + 0x80)
nrows = struct.unpack("<H", rpm(f + 0xA, 2))[0]
price = {}
for i in range(nrows):
    off = q(f + 0x40 + i * 0x18 + 8); r = f + off
    eid, p, typ, rew = u32(r), s32(r + 4), rpm(r + 0xC, 1)[0], s32(r + 0x10)
    if p > 0 and rew == 0 and typ in TAG: price[TAG[typ] | (eid & 0x0FFFFFFF)] = p
gdm = q(BASE + 0x4E140B0 - 0x4E140B0 + 0x4E1E3E8) if False else None
import subprocess, os
log = open(os.path.join(os.environ["USERPROFILE"], "ac6_roguelike_dll.log"), "rb").read().decode("utf-8", "replace")
gd = int(re.findall(r"GameData = 0x([0-9A-F]+)", log)[-1], 16)
mgr = gd + 0x600
def entry(i):
    split = u32(mgr + 0x1C)
    return q(mgr + 0x40) + i * 16 if i < split else q(mgr + 0x30) + (i - split) * 16
slots = [0x4F4, 0x4F8, 0x4FC, 0x500, 0x53C, 0x540, 0x544, 0x548, 0x550, 0x554, 0x558]
equipped = {u32(gd + o - 0xD0) for o in slots} - {0xFFFFFFFF}
name = lambda item: get(fmg_of(NAMECAT.get(item >> 28, 11)), item & 0x0FFFFFFF) or "#%d" % (item & 0x0FFFFFFF)
total = 0
print("COAM:", s32(gd + 0x74))
for i in range(min(u32(mgr + 8), 4096)):
    e = entry(i); h, item = u32(e), u32(e + 4)
    if h == 0xFFFFFFFF or h in equipped or item not in price: continue
    print("  stash  %-28s %9d" % (name(item), price[item])); total += price[item]
print("sellable stash total:", total)
