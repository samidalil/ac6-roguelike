"""price_model.py — estimate a shop price for the parts the game prices at 0 (starter parts, special parts) from
their stats, by learning how the game prices the parts it does sell.

Data: data/wiki_tables.json (fetch_data.py) for stats, the running game's EquipmentLineupParam for prices and ids
(names joined through the game's own part-name texts). Model, per part type: ridge regression of log(price) on the
standardized stats (legs / weapons also get their type as one-hot features); the ridge strength is chosen by
leave-one-out cross-validation, whose error is reported. A weapon whose other-side copy is sold keeps that price.

Output: data/suggested_prices.toml ([prices] lines, by id) + a report on stdout.
Needs the game running (reads prices live). Usage: python price_model.py
"""
import json, math, os, sys
import numpy as np
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import stash as S   # live lineup + part names (tools/stash.py)

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")
key = lambda s: "".join(c for c in s.upper() if c.isalnum())   # the wiki drops "/", ":" spacing varies

def num(s):
    s = str(s).replace(",", "").strip()
    try: return float(s)
    except ValueError: return None

# ---------------------------------------------------------------- game lineup rows
TAG = S.TAG
rows = []
for i in range(S.nrows):
    off = S.q(S.f + 0x40 + i * 0x18 + 8); r = S.f + off
    eid, p, flag, typ, rew = S.u32(r), S.s32(r + 4), S.s32(r + 8), S.rpm(r + 0xC, 1)[0], S.s32(r + 0x10)
    if typ not in TAG or rew: continue
    if typ == 0 and eid // 1_000_000 == 75: continue          # core expansions: not sellable
    item = TAG[typ] | (eid & 0x0FFFFFFF)
    rows.append({"id": eid, "kind": typ, "price": p, "flag": flag, "name": S.name(item)})

def progression(flag):
    """Features from the lineup unlock flag: 1000 = starter parts (tier 0); 6200..6280 = shop waves through the
    campaign (tier 1..9); 64xx = arena / special unlocks (tier 9); 0 = no unlock flag (its own indicator)."""
    if 6200 <= flag < 6300: t = (flag - 6200) / 10 + 1
    elif 6400 <= flag < 6500: t = 9.0
    else: t = 0.0
    return {"tier": t, "no_unlock_flag": 1.0 if flag == 0 else 0.0}

MELEE = {"Pulse Blade", "Laser Blade", "Laser Lance", "Pile Bunker", "Stun Baton", "Explosive Thrower"}
FRAME_PAGES = {"Heads": (1, 50), "Cores": (1, 51), "Arms": (1, 52), "Legs": (1, 53), "Boosters": (4, None), "FCS": (5, None), "Generators": (6, None)}
def page_of(r):
    if r["kind"] == 0: return "Weapons"
    for page, (k, pre) in FRAME_PAGES.items():
        if r["kind"] == k and (pre is None or r["id"] // 1_000_000 == pre): return page
    return None
by_page = {}
for r in rows: by_page.setdefault(page_of(r), []).append(r)

wiki = json.load(open(os.path.join(DATA, "wiki_tables.json"), encoding="utf-8"))

# ---------------------------------------------------------------- ridge regression with leave-one-out
def standardize(X):
    mu, sd = X.mean(0), X.std(0); sd[sd == 0] = 1
    return (X - mu) / sd, mu, sd

def ridge_fit(X, y, a):
    Xs, mu, sd = standardize(X)
    A = np.c_[np.ones(len(Xs)), Xs]
    I = np.eye(A.shape[1]); I[0, 0] = 0                     # no penalty on the intercept
    w = np.linalg.solve(A.T @ A + a * I, A.T @ y)
    return lambda Z: np.c_[np.ones(len(Z)), (Z - mu) / sd] @ w

def loo_preds(X, y, a):
    out = np.empty(len(y))
    for k in range(len(y)):
        m = np.ones(len(y), bool); m[k] = False
        out[k] = ridge_fit(X[m], y[m], a)(X[k:k + 1])[0]
    return out

def errors(pred, y):
    e = np.abs(np.exp(pred - y) - 1)
    return float(e.mean()), float(np.median(e))

def run(page, items, feat_fn):
    """items: list of (row, stats) with row["price"] possibly 0. Fits on priced ones, predicts the others."""
    data = [(r, feat_fn(st)) for r, st in items]
    feats = sorted({k for _, f in data for k in f})
    train = [(r, f) for r, f in data if r["price"] > 0]
    X = np.array([[f.get(k, 0.0) for k in feats] for _, f in train])
    y = np.array([math.log(r["price"]) for r, _ in train])
    # Ridge strength by leave-one-out. The ridge pulls predictions toward the mean price, so its leave-one-out
    # predictions are then calibrated (y = c0 + c1 * prediction); the reported error is leave-one-out for both steps.
    best = None
    for a in [0.1, 0.3, 1, 3, 10, 30, 100]:
        lp = loo_preds(X, y, a)
        cal = np.array([np.polyval(np.polyfit(np.delete(lp, k), np.delete(y, k), 1), lp[k]) for k in range(len(y))])
        e = errors(cal, y)
        if best is None or e[0] < best[1][0]: best = (a, e, errors(lp, y), lp)
    a, (mean_e, med_e), (raw_mean, _), lp = best
    c1, c0 = np.polyfit(lp, y, 1)
    model = ridge_fit(X, y, a)
    print(f"\n== {page}: {len(train)} priced parts, {len(feats)} features, ridge {a}: leave-one-out error mean {mean_e:.0%}, "
          f"median {med_e:.0%} (uncalibrated mean {raw_mean:.0%}); shop prices {math.exp(y.min()):,.0f} - {math.exp(y.max()):,.0f}")
    preds = {}
    for r, f in data:
        preds[r["id"]] = math.exp(c0 + c1 * model(np.array([[f.get(k, 0.0) for k in feats]]))[0])
    # Early-game anchor: the model overprices the cheapest shop parts (early-game parts with decent stats) and the
    # parts priced 0 are early-game starters; scale by the actual/model ratio of the 3 cheapest shop parts.
    cheapest = sorted(train, key=lambda t: t[0]["price"])[:3]
    anchor = float(np.median([r["price"] / preds[r["id"]] for r, _ in cheapest]))
    print(f"  early-game anchor x{anchor:.2f} (from {', '.join(r['name'] for r, _ in cheapest)})")
    return preds, anchor

# ---------------------------------------------------------------- frames / internals
suggest = []   # (id, price, name, page, note)
for page in FRAME_PAGES:
    table = wiki[page]
    cols = sorted({c for w in table for c in w} - {"#", "Icon", "Name", "Manufacturer", "Price (COAM)", "Type"})
    lineup = {key(r["name"]): r for r in by_page.get(page, [])}
    items = []
    for w in table:
        r = lineup.get(key(w["Name"]))
        if not r: print(f"  [{page}] not in the game lineup: {w['Name']}"); continue
        st = {c: num(w.get(c, "")) or 0.0 for c in cols}
        if "Type" in w: st["type=" + w["Type"]] = 1.0
        st.update(progression(r["flag"]))
        items.append((r, st))
    preds, anchor = run(page, items, lambda st: st)
    for r, _ in items:
        p = preds[r["id"]]
        if r["price"] > 0: continue
        suggest.append((r["id"], p * anchor, r["name"], page, f"model {p:,.0f} x anchor"))
    worst = sorted(((abs(preds[r["id"]] / r["price"] - 1), r["name"], r["price"], preds[r["id"]]) for r, _ in items if r["price"] > 0), reverse=True)[:3]
    for e, n, real, p in worst: print(f"  largest misses: {n}: price {real}, model {p:,.0f} ({e:.0%})")

# ---------------------------------------------------------------- weapons
wrows = by_page.get("Weapons", [])
by_key = {}
for r in wrows: by_key.setdefault(key(r["name"]), []).append(r)
items, seen = [], set()
manual = json.load(open(os.path.join(DATA, "manual_stats.json"), encoding="utf-8"))["Weapons"]
for page in ["R-Arm+Units", "L-Arm+Units", "R-Back+Units", "L-Back+Units", "manual"]:
    for w in (manual if page == "manual" else wiki[page]):
        k = key(w["Name"])
        if k in seen: continue
        seen.add(k)
        rs = by_key.get(k)
        if not rs: print(f"  [Weapons] not in the game lineup: {w['Name']}"); continue
        priced = [r for r in rs if r["price"] > 0]
        st = {"log_atk": math.log1p(num(w["ATK Power"]) or 0), "log_impact": math.log1p(num(w["Impact"]) or 0),
              "log_rounds": math.log1p(num(w["Total Rounds"]) or 0), "weight": num(w["Weight"]) or 0, "en": num(w["EN Load"]) or 0,
              "type=" + w["Type"]: 1.0, "melee": 1.0 if w["Type"] in MELEE else 0.0}
        rep = dict(priced[0] if priced else rs[0]); rep["all"] = rs
        st.update(progression(rep["flag"]))
        items.append((rep, st))
preds, anchor = run("Weapons", items, lambda st: st)
for r, _ in items:
    p = preds[r["id"]]
    priced = [x for x in r["all"] if x["price"] > 0]
    for x in r["all"]:
        if x["price"] > 0: continue
        if priced: suggest.append((x["id"], priced[0]["price"], x["name"], "Weapons", f"other side sold for {priced[0]['price']} (model {p:,.0f})"))
        else: suggest.append((x["id"], p * anchor, x["name"], "Weapons", f"model {p:,.0f} x anchor"))
worst = sorted(((abs(preds[r["id"]] / r["price"] - 1), r["name"], r["price"], preds[r["id"]]) for r, _ in items if r["price"] > 0), reverse=True)[:3]
for e, n, real, p in worst: print(f"  largest misses: {n}: price {real}, model {p:,.0f} ({e:.0%})")

# ---------------------------------------------------------------- output
print("\n== suggested values for the parts the game prices at 0")
lines = ["# Generated by tools/price_model/price_model.py: estimated values for the parts the game prices at 0.",
         "# id = price    # name (type, source)"]
for eid, p, n, page, note in sorted(suggest, key=lambda s: (s[3], s[0])):
    v = int(round(p / 1000.0)) * 1000
    print(f"  {eid:>9} {n:<26} {page:<10} {v:>8,}   {note}")
    lines.append(f"{eid} = {v}    # {n} ({page}, {note})")
open(os.path.join(DATA, "suggested_prices.toml"), "w", encoding="utf-8").write("\n".join(lines) + "\n")
