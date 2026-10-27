"""tier_lists.py — part rarity from community tier lists and "best parts" guides, as [part_rarity] lines.

WEAPONS: consensus of dated tier lists, S=5 A=4 B=3 C=2 D=1, weighted by recency:
  Game-Rack, Strongest Weapon Tier List (Patch 1.09)                        weight 3
    https://game-rack.com/en/articles/en-ac6-strongest-weapon-tier-list-patch-109-990
    (UrGameTips' 2026 list is identical: not counted twice)
  MetaTierList, Armored Core 6 Tier List (updated 2024-03-04), 76 weapons    weight 2
    https://metatierlist.com/armored-core-6-tier-list/
  Games Horizon, Best Weapons Tier List (updated 2023-11-24)                weight 1
    https://gameshorizon.com/guides/armored-core-6-best-weapons-tier-list/
  GameRant, Weapon Tier List (patch 1.02, 2023-09-14; "in general" groups skipped)  weight 1
    https://gamerant.com/armored-core-6-weapon-tier-list-best-primary-weapons-ac6/
  + 0.25 per "best weapons" guide recommending it (GGRecon = GoSuNoob, Digital Trends), at most +0.5.
  Score >= 3.75 -> epic, >= 2.75 -> rare, below -> common.
FRAMES / INTERNALS (no tier lists exist): number of independent guides recommending the part
  (GGRecon best parts [= GoSuNoob], Digital Trends best gear, Push Square best armor, PCGamesN frame / inner parts,
  GameRant best cores ranked, GGRecon best generators): 3+ -> epic, 1-2 -> rare.
Unlisted parts all go in the config's unlisted_tier pool (common by default).
Names are checked against Smithbox's English row names (offline; the DLL matches names ignoring case and punctuation).
Usage: python tier_lists.py > data/part_rarity.toml, then paste it into dll/src/default_config.toml [part_rarity]
"""
import json, os, sys
from collections import defaultdict
# offline part names: Smithbox's English row names (no game needed)
NAMES = r"D:\samee\Workspace\Modding\PC\From Software\Smithbox_2_2_5_2026_08_29_a\Assets\PARAM\AC6\Param Row Names\English"

V = {"S": 5, "A": 4, "B": 3, "C": 2, "D": 1}
WEAPON_LISTS = [
    ("Game-Rack 1.09", 3, {
        "S": ["SG-027 ZIMMERMAN", "PB-033M ASHMEAD", "VE-60SNA"],
        "A": ["DF-GA-08 HU-BEN", "SONGBIRDS", "HI-32: BU-TT/A"],
        "B": ["MG-014 LUDLOW", "VE-66LRA"]}),
    ("MetaTierList 2024", 2, {
        "S": ["VE-60SNA", "SG-027 ZIMMERMAN", "DF-GA-08 HU-BEN"],
        "A": ["SONGBIRDS", "FASAN/60E", "VP-60LCS", "HML-G2/P19MLT-04", "BML-G2/P05MLT-10", "BML-G2/P17SPL-16", "VE-66LRA",
              "VP-66LR", "Vvc-760PR", "IA-C01W1: NEBULA", "IA-C01W6: NB-REDSHIFT", "HG-004 DUCKETT", "IA-C01W2: MOONLIGHT",
              "HI-32: BU-TT/A"],
        "B": ["BML-G1/P07VTC-12", "BML-G3/P05ACT-02", "EL-PW-01 TRUENO", "EARSHOT", "SB-033M MORLEY", "WB-0000 BAD COOK",
              "RF-025 SCUDDER", "MA-J-200 RANSETSU-RF", "MA-J-201 RANSETSU-AR", "MA-E-210 ETSUJIN", "MAJESTIC", "VE-60LCA",
              "VP-66LH"],
        "C": ["VP-60LT", "WS-1200 THERAPIST", "VP-66LS", "MA-T-222 KYORAI", "44-142 KRSV", "IB-C03W3: NGI 006",
              "45-091 JVLN BETA", "DF-ET-09 TAI-YANG-SHOU", "DF-BA-06 XUAN-GE", "DF-MG-02 CHANG-CHEN", "SG-026 HALDEMAN",
              "BML-G1/P03VTC-08", "BML-G2/P16SPL-08", "Vvc-706PM", "44-141 JVLN ALPHA", "BML-G3/P04ACT-01",
              "BML-G2/P03MLT-06", "RF-024 TURNER", "MG-014 LUDLOW", "PB-033M ASHMEAD", "IRIDIUM", "LITTLE GEM",
              "LR-037 HARRIS"],
        "D": ["LR-036 CURTIS", "Vvc-70VPM", "Vvc-703PM", "EULE/60D", "SI-27: SU-R8", "SI-29: SU-TT/C", "VP-61PB", "VP-61PS",
              "BML-G1/P31DUO-02", "BML-G1/P32DUO-03", "BML-G1/P20MLT-04", "BML-G1/P01VTC-04", "HG-003 COQUILLETT",
              "VE-67LLA", "MA-E-211 SAMPU", "HI-16: GU-Q1", "HI-18: GU-A2", "WR-0777 SWEET SIXTEEN", "WUERGER/66E",
              "VP-67EB", "Vvc-700LD", "MA-T-223 KYORIKU", "Vvc-770LB"]}),
    ("Games Horizon 2023", 1, {
        "S": ["SG-027 ZIMMERMAN", "DF-GA-08 HU-BEN", "IA-C01W6: NB-REDSHIFT", "VE-60SNA", "SONGBIRDS", "PB-033M ASHMEAD",
              "VE-67LLA", "IB-C03W2: WLT 101"],
        "A": ["VP-66LH", "IA-C01W1: NEBULA", "WS-5000 APERITIF", "FASAN/60E", "VP-60LCS", "HI-32: BU-TT/A", "VP-67LD",
              "VP-67EB", "Vvc-774LS", "DF-ET-09 TAI-YANG-SHOU"],
        "B": ["VP-66LR", "HML-G2/P19MLT-04", "WB-0000 BAD COOK", "Vvc-760PR", "VP-66LS", "Vvc-706PM", "BML-G2/P05MLT-10",
              "VP-60LCD", "EL-PW-01 TRUENO", "Vvc-770LB", "IA-C01W7: ML-REDSHIFT", "WB-0010 DOUBLE TROUBLE"],
        "C": ["HG-004 DUCKETT", "IB-C03W1: WLT 011", "SG-026 HALDEMAN", "EL-PW-00 VIENTO", "SB-033M MORLEY",
              "BML-G1/P07VTC-12", "EARSHOT", "Vvc-70VPM", "IB-C03W3: NGI 006", "IA-C01W2: MOONLIGHT"],
        "D": ["DF-MG-02 CHANG-CHEN", "RF-025 SCUDDER", "WUERGER/66E", "LR-037 HARRIS", "HI-16: GU-Q1",
              "MA-J-201 RANSETSU-AR", "IA-C01W3: AURORA", "KRANICH/60Z", "45-091 ORBT", "44-143 HMMR"]}),
    ("GameRant 1.02", 1, {
        "S": ["SG-027 ZIMMERMAN", "DF-GA-08 HU-BEN"],
        "A": ["WS-5000 APERITIF", "IA-C01W1: NEBULA", "IA-C01W7: ML-REDSHIFT", "SONGBIRDS", "DF-GR-07 GOU-CHEN"],
        "B": ["WB-0000 BAD COOK", "SG-026 HALDEMAN", "VP-66LS", "VP-60LCS", "VP-66LH"],
        "C": ["EL-PW-00 VIENTO", "PB-033M ASHMEAD", "HG-004 DUCKETT", "MG-014 LUDLOW", "RF-025 SCUDDER",
              "MA-J-201 RANSETSU-AR", "MA-J-200 RANSETSU-RF", "MA-E-210 ETSUJIN"],
        "D": ["LR-036 CURTIS", "DF-MG-02 CHANG-CHEN", "RF-024 TURNER", "LR-037 HARRIS", "WUERGER/66E", "SI-27: SU-R8",
              "SI-29: SU-TT/C", "VP-61PB", "VP-61PS"]}),
]
WEAPON_RECS = [  # "best weapons" guides without tiers
    ("GGRecon/GoSuNoob best parts", ["SG-027 ZIMMERMAN", "Vvc-760PR", "RF-025 SCUDDER", "HI-18: GU-A2", "44-141 JVLN ALPHA",
        "44-143 HMMR", "WB-0000 BAD COOK", "PB-033M ASHMEAD", "HI-32: BU-TT/A", "Vvc-770LB", "VE-67LLA", "SONGBIRDS",
        "Vvc-700LD", "SB-033M MORLEY", "Vvc-706PM", "Vvc-703PM", "SI-27: SU-R8"]),
    ("Digital Trends best gear", ["SG-026 HALDEMAN", "SG-027 ZIMMERMAN", "DF-GA-08 HU-BEN", "VP-60LCS", "FASAN/60E", "VE-60SNA"]),
]
FRAME_RECS = [
    ("GGRecon/GoSuNoob best parts", ["HD-033M VERRILL", "HD-012 MELANDER C3", "DF-HD-08 TIAN-QIANG", "IA-C01H: EPHEMERA", "VE-44B",
        "BD-011 MELANDER", "DF-BD-08 TIAN-QIANG", "VE-40A", "07-061 MIND ALPHA", "AR-011 MELANDER", "AR-012 MELANDER C3",
        "AC-2000 TOOL ARM", "IA-C01A: EPHEMERA", "VE-42A", "KASUAR/42Z", "RC-2000 SPRING CHICKEN", "VP-424",
        "EL-TL-11 FORTALEZA", "VE-42B", "ALULA/21E", "FLUEGEL/21Z", "IA-C01B: GILLS", "FC-006 ABBOT", "FCS-G2/P05", "VE-21B",
        "DF-GN-02 LING-TAI", "DF-GN-08 SAN-TAI", "VP-20D"]),
    ("Digital Trends best gear", ["HD-012 MELANDER C3", "IA-C01H: EPHEMERA", "HD-033M VERRILL", "DF-BD-08 TIAN-QIANG",
        "CC-2000 ORBITER", "DF-AR-08 TIAN-QIANG", "DF-AR-09 TIAN-LAO", "AA-J-123 BASHO", "LG-022T BORNEMISSZA",
        "LG-033M VERRILL", "FC-006 ABBOT", "VE-21A", "VP-20D", "AG-E-013 YABA"]),
    ("Push Square best armor", ["HD-012 MELANDER C3", "VE-44A", "AH-J-124 BASHO", "DF-BD-08 TIAN-QIANG", "07-061 MIND ALPHA",
        "VE-40A", "DF-AR-08 TIAN-QIANG", "VE-46A", "DF-AR-09 TIAN-LAO", "VP-424", "LG-022T BORNEMISSZA", "LG-033M VERRILL"]),
    ("PCGamesN frame/inner parts", ["VE-44B", "HD-033M VERRILL", "07-061 MIND ALPHA", "VE-40A", "NACHTREIHER/46E",
        "EL-TA-10 FIRMEZA", "LG-011 MELANDER", "DF-LG-08 TIAN-QIANG", "BST-G2/P06SPD", "BC-0200 GRIDWALKER", "FCS-G2/P05",
        "FC-006 ABBOT", "VE-21A", "VP-20C", "IB-C03G: NGI 000"]),
    ("GameRant best cores", ["CS-5000 MAIN DISH", "07-061 MIND ALPHA", "IA-C01C: EPHEMERA", "NACHTREIHER/40E", "VE-40A",
        "DF-BD-08 TIAN-QIANG"]),
    ("GGRecon best generators", ["DF-GN-06 MING-TANG", "VP-20C", "DF-GN-02 LING-TAI"]),
]
key = lambda s: "".join(c for c in s.upper() if c.isalnum())

# game names (weapons, frames, internals) to check spelling and print the game's own name
game = {}
for f in ("EquipParamWeapon", "EquipParamProtector", "EquipParamBooster", "EquipParamFcs", "EquipParamGenerator"):
    for e in json.load(open(os.path.join(NAMES, f + ".json"), encoding="utf-8"))["Entries"]:
        n = (e["Entries"] or [""])[0].strip()
        n = n.split(" - ", 1)[-1]                                   # "ASSAULT RIFLE - RF-024 TURNER (Right)"
        n = n.rsplit(" (", 1)[0] if n.endswith(")") else n          # drop the side
        if n and not (f == "EquipParamWeapon" and e["ID"] // 1_000_000 == 75): game.setdefault(key(n), n)

lines, missing = [], []
# weapons
score_sum, weight_sum, seen_in = defaultdict(float), defaultdict(float), defaultdict(list)
for src, w, tiers in WEAPON_LISTS:
    for t, names in tiers.items():
        for n in names:
            k = key(n)
            if k not in game: missing.append(f"{src}: {n}"); continue
            score_sum[k] += V[t] * w; weight_sum[k] += w; seen_in[k].append(f"{src.split()[0]} {t}")
bonus = defaultdict(float)
for src, names in WEAPON_RECS:
    for n in names:
        k = key(n)
        if k not in game: missing.append(f"{src}: {n}"); continue
        bonus[k] = min(0.5, bonus[k] + 0.25)
weapon_rows = []
for k in set(score_sum) | set(bonus):
    score = (score_sum[k] / weight_sum[k] if weight_sum[k] else 3.0) + bonus[k]
    tier = 3 if score >= 3.75 else 2 if score >= 2.75 else 1
    weapon_rows.append((tier, score, game[k], ", ".join(seen_in[k]) + (f", +{bonus[k]:.2f} recs" if bonus[k] else "")))
# frames / internals
mentions = defaultdict(list)
for src, names in FRAME_RECS:
    for n in names:
        k = key(n)
        if k not in game: missing.append(f"{src}: {n}"); continue
        if src not in mentions[k]: mentions[k].append(src)
frame_rows = [(3 if len(m) >= 3 else 2, len(m), game[k], "; ".join(s.split()[0] for s in m)) for k, m in mentions.items()]

if missing: print("# NOT FOUND in the game:", missing, file=sys.stderr)
print("# weapons: consensus of community tier lists (tools/price_model/tier_lists.py), 3 epic / 2 rare / 1 common")
for tier, score, n, src in sorted(weapon_rows, key=lambda r: (-r[0], -r[1], r[2])):
    print(f'"{n}" = {tier}    # {score:.2f} ({src})')
print("# frames and internals: recommended by 3+ guides = epic, 1-2 = rare")
for tier, cnt, n, src in sorted(frame_rows, key=lambda r: (-r[0], -r[1], r[2])):
    print(f'"{n}" = {tier}    # {cnt} guide(s): {src}')
