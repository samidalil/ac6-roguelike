"""fetch_data.py — download the public AC6 part-stat tables used to train the price model (run once; cached in data/).

Frames / internals: Armored Core Fandom wiki tables (patch 1.09; stats + price, "N/A" for parts not sold).
Weapons: Fextralife wiki R/L arm and R/L back unit tables (type, attack power, impact, total rounds, weight, EN load;
no price there: prices come from the game's own EquipmentLineupParam, joined by name in build_dataset.py).
"""
import json, os, re, html, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")
UA = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"}

def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30) as r:
        return r.read().decode("utf-8", "replace")

def num(s):
    s = s.replace(",", "").strip()
    try: return float(s)
    except ValueError: return None

def fandom_table(page):
    """Rows of the first wikitable: dict header -> cell text."""
    url = ("https://armoredcore.fandom.com/api.php?action=parse&format=json&prop=wikitext&page="
           "ARMORED_CORE_VI_FIRES_OF_RUBICON/Parts/" + page)
    t = json.loads(get(url))["parse"]["wikitext"]["*"]
    rows = []
    for table in re.findall(r"\{\|.*?\n\|\}", t, re.S):   # each wikitable separately (legs: one per leg type)
        hdr = re.findall(r"^!'''(.*?)'''", table, re.M)
        for block in table.split("\n|-")[1:]:
            cells = [c.strip() for c in re.split(r"\n\|\|?", "\n" + block.split("\n|}")[0])[1:]]
            if len(cells) < len(hdr): continue
            row = {}
            for h, c in zip(hdr, cells[:len(hdr)]):
                c = re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", c)   # [[link|text]] -> text
                row[h] = c.strip()
            rows.append(row)
    return rows

def fextralife_table(page):
    t = get("https://armoredcore6.wiki.fextralife.com/" + page)
    tab = re.findall(r"<table.*?</table>", t, re.S)[0]
    rows = []
    for r in re.findall(r"<tr.*?</tr>", tab, re.S):
        cells = [html.unescape(re.sub(r"<[^>]+>", "", c)).strip() for c in re.findall(r"<t[hd][^>]*>(.*?)</t[hd]>", r, re.S)]
        rows.append(cells)
    hdr, body = rows[0], rows[1:]
    return [dict(zip(hdr, c)) for c in body if len(c) == len(hdr)]

if __name__ == "__main__":
    os.makedirs(DATA, exist_ok=True)
    out = {}
    for page in ["Heads", "Cores", "Arms", "Legs", "Boosters", "FCS", "Generators"]:
        out[page] = fandom_table(page)
        print(f"{page}: {len(out[page])} rows, columns {list(out[page][0])}")
    for page in ["R-Arm+Units", "L-Arm+Units", "R-Back+Units", "L-Back+Units"]:
        out[page] = fextralife_table(page)
        print(f"{page}: {len(out[page])} rows, columns {list(out[page][0])}")
    with open(os.path.join(DATA, "wiki_tables.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)
