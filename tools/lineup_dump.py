from ac6_lib import *
u32 = lambda a: struct.unpack("<I", rpm(a, 4))[0]; s32 = lambda a: struct.unpack("<i", rpm(a, 4))[0]
END = 0x7FF4D38EABA0; START = END - 318 * 0x20
print("assumed data start %X; first rows:" % START)
rows = []
for i in range(318):
    r = START + i * 0x20
    rows.append((r, u32(r), s32(r+4), s32(r+8), rpm(r+0xC,1)[0], rpm(r+0xF,1)[0], s32(r+0x10)))
for x in rows[:4] + rows[136:142]: print("   %X equip=0x%08X (%d) price=%d flag=%d type=%d trophy=%d reward=%d" % (x[0], x[1], x[1], x[2], x[3], x[4], x[5], x[6]))
print("type histogram:", {t: len([1 for x in rows if x[4]==t]) for t in sorted(set(x[4] for x in rows))})
by = {}
for x in rows: by[x[1]] = x
keys = [10000000, 10300000, 30270000, 35270000, 0x12FC0380, 0x13290560, 0x132A8C00, 0x742C4558]
print("\nflag-bypass keys -> lineup rows:")
for k in keys:
    x = by.get(k) or by.get(k & 0x0FFFFFFF)
    print("   0x%08X -> %s" % (k, ("addr=%X equip=0x%08X price=%d flag=%d type=%d trophy=%d reward=%d" % (x[0], x[1], x[2], x[3], x[4], x[5], x[6])) if x else "NOT FOUND"))
fl = {}
for x in rows: fl[x[3]] = fl.get(x[3], 0) + 1
print("\nflag histogram: %s" % dict(sorted(fl.items())))
print("flag 0 rows (%d): %s" % (fl.get(0, 0), [("0x%08X" % x[1], "t%d" % x[4], "p%d" % x[2], "rew%d" % x[6]) for x in rows if x[3] == 0]))
print("price 0 rows (%d): %s" % (len([1 for x in rows if x[2]==0]), [("0x%08X" % x[1], "flag%d" % x[3], "t%d" % x[4]) for x in rows if x[2] == 0]))
print("reward!=0 rows (%d): %s" % (len([1 for x in rows if x[6]]), [("0x%08X" % x[1], x[6]) for x in rows if x[6]]))
# id table lookup: the PARAM header precedes the data; find "EQUIPMENT_LINEUP" string and the id table to map row id -> data offset
hdrblob = rpm(START - 0x4000, 0x4000)
print("\n(id table search) offsets in 0x4000 before data start with pattern of ascending ids:", [hex(i) for i in range(0, 0x4000, 4) if struct.unpack_from("<I", hdrblob, i)[0] == 112 and struct.unpack_from("<I", hdrblob, i+4)[0] == 0][:5])
