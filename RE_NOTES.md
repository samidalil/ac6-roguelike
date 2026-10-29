# AC6 Roguelike mod — RE notes (handoff)

Game: Armored Core VI, Steam, exe **1.09** (`armoredcore6.exe`, base `0x140000000`, no ASLR observed).
Loader: **me3** with `disable_arxan = true`, separate dev save (`savefile = "AC6_roguelike_dev.sl2"`).
Tools: `tools/*.py` (CE-free memory readers + `ce_bridge.py` = run Lua inside CE from the shell, see `tools/README.md`), Cheat Engine 7.5 + **The Grand Archives AC6 table v0.4.4** (registers `GameDataMan`, `SoloParamRepository`,
`paramUtils`; `GameMan` warning is harmless), Smithbox 2.2.5 (AC6 project), `ac6_session.lua` (this repo).

## Design (decided)
- Runtime DLL (Rust, me3 `[[natives]]`), edits params **in memory** — no shipped regulation.bin (compat + install simplicity).
- Shop = Slay-the-Spire style: 1 weapon + 2 frame parts, rerolled after each mission.
- Missions give an item instead of COAM; inventory cap ~20–25; full inventory = reward forfeited (+ player message).
- Sell for low price; death tax (tunable, not 60% by default); hide Replay Mission; arena rematch cash = 0.
- **Economy decided 2026-10-26:** mission pay = 0 (MissionParam `currencyReward` + all event/enemy objective bonuses),
  ammo & repair COSTS KEPT (`ammoCostMultiplier`/`damageCostMultiplier` untouched) → missions cost money, selling parts
  is the only income. **COAM < 0 = GAME OVER → the run's save is reset.**
- ~~Death tax at the moment of death~~ (implemented + verified 2026-10-26, then REPLACED the same day by:)
- **Failure rule decided 2026-10-26:** no pause menu during missions (no settings, no Retry, no Quit). Death sends the
  player straight back to the garage (auto Quit). An ESCROW event flag is set in the save at sortie and cleared when
  the mission is cleared. When the garage loads with the flag still set (death, or Alt-F4 mid-mission), the flag is
  consumed and random parts are removed from the player, with a dialog listing them. **No COAM penalty on death.**
  Mission costs (ammo/repair) still apply on clears; COAM < 0 = game over.
- **Penalty rules decided 2026-10-26 (applied when the garage opens with the escrow flag set):**
  - Every EQUIPPED part is affected. Weapons (arm/back units) are lost, slots left empty (fists are allowed).
  - Frame parts (head/core/arms/legs) and internals (booster/FCS/generator, assumed same rule) are lost and replaced by
    the STARTER part of that slot (re-granted if the player sold it). A slot that already holds its starter costs the
    starter's full price instead.
  - Starters get real prices = price of the cheapest shop part of the same slot type.
  - COAM < 0 → auto-sell STASH parts (not equipped), most expensive first, until >= 0. Nothing left to sell and still
    negative → GAME OVER (save reset).
  - One dialog lists everything: parts lost, starter costs paid, parts auto-sold.
- Worm Killer (VE-60SNA, rows 371/372): `price = 0` (unsellable) + not in shop until mission 3230 (Ice Worm) cleared,
  then normal. Mission-critical grants bypass the cap. Starter parts get real prices (sellable, in rotation pool).

## Next features (decided with the user 2026-10-27 — one session per task, see the task chips)
Shared rules: keep designs simple; events, not polling; kill the game before rebuilding; one game instance, so test
sessions run one at a time (the dev save AC6_roguelike_fresh3.sl2 is shared). New per-row state lives in the save as
event-flag BITSETS (one bit per EquipmentLineupParam row index, 318 rows), next to the shop state (flags 7800-7827,
7900+). Free, save-backed flag ranges must be verified first (tools/flag_test.py).

1. **Save storage + reroll cadence + starting credits.**
   - Row-bitset helper over a verified flag range (read/write/clear a set of row indices).
   - Config `[shop] reroll_interval = 1` = missions cleared between shop rerolls (counter kept in the save; mission
     rewards are still given on every cleared mission).
   - Config `[economy] starting_credits = 100000`, given once per run at the first mission end (the moment the first
     shop is rolled: no shop state in flags yet). Runs already in progress do not get it.
2. **Buy-back until the reroll.** Every part sold since the last reroll (Vente sales AND automatic sales) is offered
   again in the shop at its normal shop price (value x item_charge_percent), until the next reroll; kept in the save
   (bitset "sold since reroll"), no size limit. Left and right weapon versions are different items/rows: a sold
   right version comes back as the right version only. Parts lost to a failure never enter it. Detect sales through
   the game's sell function (find it; the sale-value getter exe+B885D0 is called from the Vente menu at 0xD2FDDA /
   0xD303DD), not by diffing the inventory.
3. **Starter parts always buyable + failure rework.**
   - The 7 starter frame parts/internals (FRAME_SLOTS in penalty.rs: head, core, arms, legs, booster, generator,
     FCS) are always in the shop while not owned, outside the random offers (not the starter weapons).
   - Failure (death / quit mid-mission), still one deterministic FailurePlan built at sortie:
     a. every equipped weapon is lost forever (bitset "lost forever");
     b. every equipped frame part / internal is lost, a starter included (a starter is never "lost forever": always
        buyable); non-starter ones go to "lost forever";
     c. each emptied slot gets the MOST RECENTLY OBTAINED owned spare for that slot (inventory entry +0xC looks like
        an acquisition ordinal — confirm against the assembly's "obtained" sort; fallback most expensive); with no
        spare, the starter is bought (charged its shop price) and equipped;
     d. tank legs have no booster: if the resulting legs are tanks, the booster slot stays empty and nothing is
        bought; non-tank legs with an empty booster slot get a spare or a bought starter booster;
     e. credits - starters bought < 0 -> auto-sell unequipped parts, most expensive first, never the parts picked as
        replacements; auto-sold parts go to the buy-back list; still < 0 with nothing left = game over.
        Single pass (replacements first, then cost, then sales): no loop possible.
     f. equipped parts are never auto-sold (garage auto-sell after a cleared mission too): the AC stays complete.
     g. check that Vente cannot sell equipped parts.
4. **Pool without repeats (TBOI-style)** — replaces progressive rarity odds. *(Removed 2026-10-28: duplicates allowed, see "Duplicate parts in the
   shop".)*
   - Every rolled part (shop offer or mission reward) leaves the pool (bitset "rolled", in the save).
   - Two pools: weapons and other parts (the shop rolls them separately). When one cannot fill its shop slots, take
     what is left, refill that pool with every part not owned and not lost forever, and complete from the refill
     without repeating what was just picked. Mission rewards draw from both.
   - Nothing ever rolls an owned part (no duplicates). No candidate at all: the shop slot stays empty; a mission
     reward instead rolls among everything (owned included) and pays that part's sell value in credits.
5. **Worm Killer (VE-60SNA, rows 371/372).** Find how/when vanilla grants it (script grant, rewardCondition 0). It
   becomes the reward of the mission just before the Ice Worm fight (mission 3230), replacing one random drop;
   given even when parts_per_mission = 0. Sellable like any part. Must not be granted twice.

## Param facts (verified in Smithbox CSV exports)
- `EquipmentLineupParam` (318 rows) is the real shop table. Row layout (bytes): `+0 equipId s32`, `+4 price s32`,
  `+8 eventFlag_forRelease u32`, `+C equipType u8` (0 weapon,1 frame,4 booster,5 FCS,6 generator), `+F isTrophyTarget`,
  `+10 rewardCondition u32`.
  - `price = 0` ⇒ unsellable **and** unpriced/free in buy list (22 rows: LOADER 4 starters, loaners, expansions). Verified live.
  - `price` is read from the param **every frame** by the shop UI (readers `exe+E84699`, `exe+435B86`) — live patching works.
  - Release flags are coarse waves (6200/6230/6250/6260…); flag `0` = never in shop; `1000` = set at new game. `-1` is
    treated as `0` by the ctor (`cmove`).
  - `rewardCondition` only used for rank rewards (1–15) and arena/test (30–34); VE-60SNA has 0 (granted by mission script).
- `ShopLineupParam` is vestigial (value −1, flag 1000 everywhere). Ignore.
- `UnlockParam_MenuCommand` (`GameDataUnlockParam`): all `unlockType 2` (event flag). **Replay Mission = row 10072, flag 3134**.
  Market 3127, AC Test 3126, OS Update 3131.
- `ArenaParam`: `firstReward_cash` / `rematchReward_cash` (rematch = 50 % of first). Zero the rematch field (41 rows).
- `MissionParam`: `baseReward`, reward/deduction line items (`TextID`/`UnitPrice`), `bulletCost_rate`,
  `repairCost_coefficient`, `mailID_0..4`, `assemID`. Mission ids e.g. 2030 Wallclimber, 3230 Ice Worm, 3500 Historic Data
  Recovery, 4200 Escape.
- Row ids: FINDER EYE 112, RANSETSU-RF 184/185, VE-60SNA 371/372.

## Memory / code map (1.09)
| Address | What |
|---|---|
| `[GameDataMan]` | TGA symbol. `+0x08` = player GameData, `+0x40` = `CSEquipmentLineupData` |
| `GameData+0x230` | player items object (inventory); `exe+46E180` on it -> the table `exe+474060` searches |
| `exe+434FE0..4352xx` (start = prologue `mov [rsp+8],rcx; push rbp..r15; sub rsp,108`) | `CSEquipmentLineupData` ctor/init: caches all lineup rows into std::maps (MSVC node: `+19 isnil`, `+1C key`), incl. a map keyed by release flag. Reads `eventFlag_forRelease` at `exe+4352D6`. Runs on title-screen transitions and Continue, **not** on mission/arena return. Source file string: `..\..\Source\GameData\CSEquipmentLineupData` |
| `exe+435B50` / read at `+435B86` | lineup price getter (`mov eax,[rax+4]`) |
| `exe+435C90` | copies `item[+0x54]` (gaitem id) to `item[0]`, returns &item |
| `exe+B885CF` | `CSGaitemAccessor::GetPrice`-like: dispatch on id top nibble (`shr 0x1C`), parts → `[GameDataMan]+0x40` → `+435B50` |
| `exe+E87F80` | shop menu per-frame function: `+E87FD8 call +E88180(menuObj, &handle)` → RAX = handle; `+E87FE9 call +E928F0(list, ?, &filter)`. **`+E88180` rebuilds the whole list every frame** (see Answer) |
| `exe+E88180` | `jmp exe+641901C` (Arxan-relocated body). Fills a stack handle; `[handle+0]` = persistent container |
| container (`7FF39A7A33F0` last session) | `+0 vtable exe+395CE18`, `+8 refcount`, `+10 vtable`, `+18 begin`, `+20 end`, `+28 cap` |
| shop list entries | stride **0x118**, 259 entries. `+10 vtable exe+390BA08`, `+20 price`, `+28 shared ptr`, `+50 int`, `+54 gaitem id` (`0x10000000 \| equipId`), `+58 ordinal`, `+60` ptr to a 0x20-stride **stats param row (not the lineup row; shared by the ±0x100 variants)**, `+68/+6C/+70 ints`, `+74 byte` |
| `exe+E928F0` | per-frame loop: `for i<count: item=src[i]; if filter->Accept(item) push(item)` (`+E929B4 call [rax+10]`; filter = std::function, callable at [r13+0x38]) |
| `exe+E88B10` | _Do_call of a function-pointer std::function (vtable exe+395BF20): `jmp [callable+8]` |
| `exe+E89820` | the buy list's OWNERSHIP filter (built at exe+641D108): `+D2F860(+435C90(item)) > 0`, i.e. not owned (the old "NEW-badge" label was wrong; it returned 0 = owned for every entry on the dev save) |
| `exe+D2F900` | shop "obtainable": `1 if +474060(inventory, &id) < 0` (not owned) else 0. See "Duplicate parts in the shop" |
| `exe+5F09B0B` / `+5F09B25` | entry copy routine (Arxan region): `[rdi+50..74] = [rbx+50..74]`, then `jmp exe+E838A8`. Writes every entry **every frame** (per-frame projection of a persistent source). Also used stack→stack elsewhere — condition on `RDI == entry addr` |

## Tests done
- Changing `eventFlag_forRelease` in the param: no effect on open shop (cache), never read by shop/mission end; read on title/Continue.
- Changing `price`: immediate in buy/sell menus. `0` ⇒ unsellable; FINDER EYE with price >0 ⇒ sellable **and buyable** (starters are in the lineup).
- Access watch on `eventFlag_forRelease`: 0 on shop, 2 on exit-to-title, 2 on title→menu, 1 on Continue, 0 on mission/arena finish.

## Answer to the open question (session 2026-09-16, exe 1.09)

**There is no persistent shop list.** The 259-entry vector is a *per-frame projection*: `+E88180` (body relocated to
`exe+641901C`) rebuilds it every frame while the shop is open, via `+E876B0 → +E87A10 → +E873D0 → +E88050 → +E88F00`
(the real builder) `→ +E881D0 → +E89B80 → +E837C0` (entry copy, Arxan body at `+5F09B0B`). Each entry is built on the
stack, copied through a pool-allocated temp (`7FF46F011B80`; the pool free-list head lives at `[exe+4CBE7C8]`), then into
the vector. Closing the shop destroys the container (no object with vtable `exe+395CE18` remains). The container address
therefore changes per open (`7FF39A7A33F0` → `7FF39A7A3270` today).

**The switch is the event flag, evaluated every frame.** Per-item predicate `exe+159ACB0(itemCtx)` (called from `+E88F00`):
```
lineup = [GameDataMan]+0x40
if lineup && CSEquipmentLineupData_map8_lookup(lineup, equipId)   // +435C40: bool map at lineup+0x08
    return 1                                                     // shown regardless of flags / rewardCondition
row = [itemCtx+8]                                                // EquipmentLineupParam row layout (+0 equipId +4 price +8 flag +10 reward)
if !row || row.rewardCondition > 0  return 0                     // rank/arena rewards never in shop
flag = row.eventFlag_forRelease; if flag == -1: flag = 0
if flag == 0  return 0                                           // flag 0 = never
return CSEventFlagMan::IsEventFlag(flag)                         // +AD8260(man, &flag) → +B07CF0(man, flag)
```
Consequences for the mod:
1. Toggling a release flag shows/hides its whole wave immediately (no reload). Live toggle test is written
   (`tools/flag_test.py`, `%TEMP%\ac6_step9.lua`) but **not yet run** — the workstation locked before the shop was reopened.
2. Per-item control = give every row a private flag id (patch `eventFlag_forRelease` in the param; the check reads the row
   pointer held by the item ctx — whether that is the live param row or a ctor copy is the one thing still unverified,
   see "Still to verify"). Waves in the shipped param: 0 (51 rows), 1000 (10), 6200…6280 (12 waves), 64xx pairs (2 rows each).
3. The bool map at `CSEquipmentLineupData+0x08` (318 keys = gaitem ids `cat<<28|equipId`, **8 true** in the dev save:
   weapons 10000000/10300000/30270000/35270000, frames 0x12FC0380 (starter head, price 0, flag 1000)/0x13290560/
   0x132A8C00, FCS 0x742C4558) bypasses the flag check entirely. Semantics TBD (looks like "currently equipped" or
   "script-granted"); this is almost certainly why the earlier FINDER EYE / RANSETSU flag edits and access watches
   showed nothing — those rows never reach the flag read.

### CSEventFlagMan (found via reflection registration, not RTTI — the binary has RTTI but not for this class)
| Address | What |
|---|---|
| `exe+4E13070` | global → instance (`7FF46EE808C0`, 0x1C0 bytes; ctor `exe+AD6BB0`, allocated at `exe+B8F2EC`, stored at `exe+B8F30D`) |
| `exe+B07CF0` | `IsEventFlag(this, uint id) → int` (0/1) |
| `exe+B08500` | `SetEventFlag(this, uint id, bool state)` (also sets dirty byte `this+0x60`) |
| `exe+B07FE0` | `GetEventFlagValue(this, id, bits)` (wrappers `+AD8280` 32-bit, `+AD82E0` 8-bit) |
| `exe+AD8260` | `IsEventFlag(this, uint* id)` wrapper used by the shop |
| layout | `+0x1C` divisor 1000, `+0x20` block size 0x7D, `+0x24` block count 0x280, `+0x28` block base, `+0x38` std::map block→page (node `+0x19 isnil, +0x20 key, +0x28 type(1=index,2=ptr), +0x30 value`); bit = `page[(id%1000)>>3] & (0x80 >> (id%1000 & 7))` |
Verified reads on the dev save: 1000/3126/3127/3131/3134/6200/6230/6250/6260 = 1, 6300 = 0.

### CSEquipmentLineupData (`[GameDataMan]+0x40`, `7FF46EF41400`)
`+0x08` map gaitemId→bool (318, see above) · `+0x20` map gaitemId→? (318) · `+0x38` map gaitemId→cache obj (306) ·
`+0x60` map releaseFlag→row list (29 keys: 0, 1000, 6200, 6210, 6220, 6230, 6240, 6245, …) · `+0x78` map key 80…138 (21) ·
`+0xA0` container queried by `+444B10` from `+435CA0` (sold-out / limit check?). Map node: `+0 left, +8 parent, +10 right,
+19 isnil, +1C key(u32), +20 value`.

### EquipmentLineupParam in memory (SoloParamRepository copy, live values)
Data start = `"EQUIPMENT_LINEUP_PARAM"` type string − 318·0x20 (`7FF4D38E83E0` this run; address changes per launch).
Row stride 0x20. Frame/booster/FCS/generator rows store the **prefixed** id (`0x02FC0380`-style) in `+0`; the map keys add
the category nibble (`0x12FC0380`). Populations: type 0 = 198, 1 = 84, 4 = 12, 5 = 10, 6 = 14; price 0 = 22 rows (matches
Smithbox); rewardCondition ≠ 0 = 21 rows.

### Verified live (session 2026-09-16, late; exe 1.09, dev save, buy list open on the R-arm tab)
1. **Live toggle — CONFIRMED.** Clearing event flag 6200 (bit write, `tools/flag_test.py --set-flag 6200 0`) dropped the
   per-frame source list from **259 → 231** entries within one frame (= the 28 non-bypass rows of wave 6200); restoring it
   brought back 259. RANSETSU-RF (bypass map) stayed listed throughout. Counted inside the `exe+E87FDE` trap
   (`ac6.snapCount(tag)`, `%TEMP%c6_step13.lua`) because the 259-entry container is **rebuilt every frame and freed
   again** (its address alternated `7FF39A7A1CE0` / `7FF39A7A3450`); a CE-free memory scan only ever sees the
   *persistent* 1-entry projection (same vtable `exe+395CE18`) that feeds the visible list.
2. **Row pointer origin — CONFIRMED live param.** Step 8 trap at `exe+159AD16` hit with `RAX = 7FF4D38E83E0` = data start of
   the SoloParamRepository copy of `EquipmentLineupParam`. Patching `eventFlag_forRelease` in that table is therefore
   picked up every frame, no ctor re-run needed. Private per-row flags work.
3. **Predicate reconstruction — CONFIRMED.** Evaluating the predicate in Python over all 318 rows (`tools/shop_state.py`:
   bypass map → rewardCondition → flag 0/-1 → event-flag bit) gives exactly 259, the captured source count.
   Breakdown: 38 rows flag 0, 13 reward-only, 9 wave 1000, 6200:28+2 bypass, 6210:12+1, 6220:12, 6230:32, 6240:27+1,
   6245:7+1, 6250:44+2, 6260:32, 6275:11, 6280:11, 64xx pairs (one row of each pair is reward-locked).
   Event flags on the dev save: 1000/3126/3127/3131/3134/6200/6210/6220/6230/6240/6245/6250/6260/6280 = 1, 6270/6300 = 0.
4. **Ownership filter (new).** The visible R-arm list shows only RANSETSU-RF although 180 weapons pass the release predicate:
   the per-frame filter loop (`exe+E928F0`, `filter->Accept`) drops everything the save already owns (this dev save owns
   nearly all parts). RANSETSU-RF is listed because the bypass map forces it in; entry fields: `+6C = lineup row id`
   (184), `+68 = 4`, `+70 = 1`, `+74 = 1`, `+58 ordinal 130`, `+60` → `7FF4D38EC3E0` (stats param, past the lineup table).
   Consequence for the mod: the shop's "unowned" filter is already what a roguelike wants; a rerolled 3-item shop only
   needs 3 rows with their private flags set.
5. **`SetEventFlag` (`exe+B08500`) works when called from outside** (CE `executeCodeEx(0,nil,fn,man,id,state)`):
   6270 went 0 → 1 → 0, dirty byte `man+0x60` set. `IsEventFlag` likewise. The DLL can call both directly.
6. **Param id table layout (TGA-free row lookup).** 64-bit PARAM: header 0x40 (`+0xA u16 rowCount = 318`,
   `+0 strOff`), then 318 × 0x18 id entries `(id u32, pad u32, dataOff u64 from header, nameOff u64)`, then row data
   (`header + 0x1E10`). This run: header `7FF4D38E65D0`, data `7FF4D38E83E0`. `ac6.locateLineup()` (AOB-scan for
   `"EQUIPMENT_LINEUP_PARAM "`, writable non-exec) now backs `ac6.row(id)` when `paramUtils` is nil.
   Row addresses: 112 → `+8D20` (idx 74, price 0, flag 1000), 184/185 → `+9560/+9580` (flag 6200), 371/372 →
   `+AA80/+AAA0` (flag 6250), 34 → `+8460` (first source entry, head 0x12FAF080).

### Resolved: the 20 "released but not shown" rows (session 2026-09-16, late)
The buy list gate is **released && !owned**, and every one of the 20 is genuinely not-shown for a mundane reason:
- **10 are price-0 flag-1000 loaner/tutorial parts** — the LOADER "2000" starter AC (HC/CC/AC/2C-2000 FINDER EYE /
  ORBITER / TOOL ARM / CRAWLER) plus the opening loaner weapons/internals (RF-024 TURNER, HI-32 BU-TT/A,
  BML-G1/P20MLT-04, AG-J-098 JOSO, BST-G1/P10, FCS-G1/P01). Price 0 ⇒ never a real buy-list item (matches the earlier
  "price 0 = unpriced/free, 22 rows" finding). These are the intro/cutscene IDs.
- **10 are the LEFT/RIGHT mount twin of a weapon the save already owns.** EquipmentLineupParam carries a separate row per
  hand (Right = 10xxxxxx/30xxxxxx, Left = 15xxxxxx/35xxxxxx, +0x5000000). Ownership is tracked per exact row, so the
  save owns one hand's row and the opposite-hand row reads owned=False — but the weapon is fully owned, so it never needs
  to reappear in the shop. Verified pairs (unowned side → owned twin): LR-036 CURTIS, MG-014 LUDLOW, VP-66LH, IA-C01W1
  NEBULA, WB-0000 BAD COOK, DF-GR-07 GOU-CHEN, IB-C03W3 NGI 006, VE-60SNA, IA-C01W3 AURORA, BML-G1/P01VTC-04 — each has
  its other-hand row in the +0xA0 owned set. So these are **duplicate mount-variant rows, not dev leftovers or shop
  entries**. (Name caveat: EquipParamWeapon.txt vs EquipmentLineupParam.txt disagree on a couple of these labels; the
  L/R pairing via ±0x5000000 + owned-twin is the reliable signal, not the name file.)
Net: there is **no mysterious third filter** — the reconstructed predicate is complete; the "missing" rows are price-0
loaners and already-owned mount twins. Tool: `%TEMP%c6_sibs.py` / `ac6_sibs2.py`.

### Still open
- **map+0x08 semantics (bypass): CLOSED — not equipment.** Swapping a part in Assembly left all 8 keys byte-identical
  (`%TEMP%c6_bypass.py`), ruling out "currently equipped." The 8 (RANSETSU-RF R, PFAU/66D R, VE-60SNA R+L, FINDER EYE
  head, NACHTREIHER/42E legs, KASUAR/42Z legs, FCS-G2/P12SML) are dev/cutscene/starter-grant entries (user confirms they
  are not the player's preset). Only mod-relevant fact: **VE-60SNA (371/372) is in this set**, so its rows bypass the flag
  check — the DLL must clear their map node directly to hide the Worm Killer, flags alone won't. Not investigated further.
- **`exe+435CA0` / `+444B10` — ANSWERED (static + live dump).** `lineup+0xA0` holds an **owned/obtained id set**
  (std::set of gaitem ids, header at `+0xA0+0x20`, node key `+0x1C`): 239 keys on the dev save, a strict subset of the
  259 released rows (167 weapons, 49 frames, 10/7/6 booster-FCS-generator). `+435CA0(lineup, bool* out, uint* id)` =
  `IsOwned(id)`, **except** ids whose `+0x08` bool-map value is true return false unconditionally (`+436257` is the plain
  bool wrapper; `+444B10` = set contains, `+444BB0` = same without the override; callers `+434666`/`+43780D` filter id
  arrays). So the buy list is `released(flag/bypass) && !IsOwned` — that is why the R-arm tab shows only RANSETSU-RF
  (owned, but bypass ⇒ treated as unowned). Predicted unowned-released on this save: 20 (13 weapons incl. the 64xx
  `+100` variants and 30270000 VE-60SNA-R, 4 frames, 1 booster/FCS/generator; 10 of them are price-0 flag-1000
  loaners). The R-arm tab shows none of its six, so **one more filter hides the loaners and 64xx variants** — check the
  other tabs' counts to pin it down (the pure-Python prediction is in the session transcript, rerun `shop_state.py`).
  Mod impact: **VE-60SNA rows 371/372 are in the bypass map** ⇒ flags alone cannot hide them; the DLL must also set
  their `+0x08` node value to 0 (map is rebuilt by the ctor on title/Continue, so re-patch after those transitions).
- TGA record 251 "ParamUtils" does **not** activate in the non-elevated CE (`Active` stays false; class `ParamUtils`
  exists, instance `paramUtils` nil). Not needed anymore thanks to `ac6.locateLineup()`.

## DLL plan (me3 native, Rust)
- Boot: wait for `SoloParamRepository`, patch rows (mission `baseReward=0`, arena rematch=0, starter prices, needle lock,
  private release flags per row — outcome 1 confirmed statically; flags are re-evaluated every frame, so SetEventFlag(`exe+B08500`) on garage return rerolls the shop with no reload).
- Garage return: reroll shop (flags or list), grant mission item (cap check), death tax on fail, keep flag 3134 off.
- Message: `MessageBoxParam` / mail via `MailParam` + FMG text package (only shipped file besides the DLL).
- Anti-tamper: rely on me3 `disable_arxan` (dearxan); do not ship own patcher.

## Session notes (2026-09-16)
- The usual CE instance runs **elevated** (manifest `highestAvailable`), which blocks automated input (UIPI). A second CE
  started with `$env:__COMPAT_LAYER='RunAsInvoker'` attaches and debugs the (non-elevated) game fine; TGA `Open` works there
  but cannot write its run-once file (harmless warnings) and `paramUtils` needs record ID 251 "ParamUtils" enabled.
- `ac6_session.lua`: traps/watches now call `debug_continueFromBreakpoint(co_run)` — CE 7.5 otherwise breaks into the
  Memory Viewer even when the callback returns 1. `ac6.verify()` = all OK on 1.09. Step scripts used this session live in
  `%TEMP%\ac6_step1..9.lua`; the log is `%USERPROFILE%\ac6_re_log.txt`.
- Two stray elevated CE instances may still be open (PIDs 4952 + the original 12752) next to the non-elevated one (3184).
- **CE from the shell (no GUI):** CE 7.5 already has its Lua pipe server open (`\.\pipe\CELUASERVER`).
  `tools/ce_bridge.py` loads `luaclient-x86_64.dll` and calls `CELUA_ExecuteFunction`; `python ce_bridge.py file.lua`
  dofile's it inside CE and prints the log lines it produced. Traps/timers fire after the call returns → sleep, re-read
  the log. One client at a time. This replaced clipboard-pasting into the Lua Engine for everything after step 10.
- Workstation auto-locks mid-session (lock screen = black screenshot); `ReadProcessMemory` tools keep working while locked,
  the GUI path does not.

## Vanilla unlock mechanism — OBSERVED on a fresh save (2026-09-21)
Method: CE-free flag-bit poller (`tools/flag_monitor.py`, no breakpoints) + a single 1-byte hardware write-watch on the byte
holding flag 6200 (`tools/ce/step15_watch_flag6200.lua`). (A CE execute-breakpoint on `SetEventFlag` is unusable: it is
called every frame — 3000→1 / 6105→0 each frame — and every hit makes CE steal focus.)
- **New game init (03:42:09):** flags 900, 1000, 2001, 2040 → 1 (1000 = "set at new game", confirmed), block of 66xx → 0.
- **Mission 1 clear (03:49:41–43):** 2550/2553/2554/2555 → 1 (mission-clear state) together with 3000, 3122, 3135, 3700;
  then 3123/3124/3125/3126(AC Test)/3134(Replay)/3139/3200/3210/3220/3409 → 1; 23 s later 3139/3200/3210 → 0, 3400, 5011 → 1.
  **No parts wave, no 3127** — the shop is not open after mission 1.
- **Mission 2 clear (03:55:14):** **3127 (Market) → 1 and 6200 (first parts wave) → 1 in the same instant**; the shop opened
  with buyable weapons immediately. No param reload; the shop shows the wave because `IsEventFlag(6200)` is now true (same
  as our live-toggle proof). No "new part" notification yet — part GRANTS (inventory) are a separate mechanism (being
  captured with `tools/owned_monitor.py`, which polls the +0xA0 owned set / +0x08 bypass map).
- **Writer of 6200 (write-watch backtrace):** `exe+B085E2` inside `SetEventFlag` (R14 = 0x1838 = 6200) ← `exe+AD4DDE`
  ← `exe+AD87F8` (wrapper `+AD87C0`) ← **`exe+179FDC9`** ← 4CDA9C8 / 293CFFE / 303C7B4 / 17A51D7 / CC4282 / 179D7FC / 2915177.
  `exe+179FD70..DC4` is a **script-VM "SetEventFlag(flag,value)" instruction handler**: typed arg (int or float via
  cvttss2si) → bool → `call +AD87C0(flagMan, &flag, state)` → `jmp +17A306D` (dispatcher continue). The same handler
  also produces the per-frame 3000/6105 writes (a script loops them). So **vanilla unlock = an event script executed at
  mission end sets the wave flag(s)**; the game logic that runs that script lives upstream (`exe+CC4240` region holds two
  real singleton globals `exe+4E15708`, `exe+4E15530`).
Consequence for the mod: to change the shop after a mission, act at mission end exactly like vanilla — set event flags
(copy-independent, proven from outside via `exe+B08500`). Per-item control still needs per-row unique flags in the LIVE
param (or the predicate call-site hook at `exe+E89074`); the flag-setting moment itself can be detected by polling the
25xx mission-clear flags / 3401 / 5011, or by hooking the script handler at `exe+179FDC4`.

## Player inventory (gaitem) — FOUND (2026-09-21, fresh save; addresses this launch, region = GameData area)
The set at `CSEquipmentLineupData+0xA0` is **NOT "owned"** — it is the **"seen in the shop" set** (NEW-badge clearing):
it grew by 13–14 ids at once during garage shop visits (04:07:03, 04:14:52 = the early-game heads/frames/weapons by tab)
and did **not** change when a mission granted a part. Correct the earlier "+0xA0 = owned / +435CA0 = IsOwned" reading:
`+435CA0` = "seen (unless bypass)". The real inventory is the **CSGaitem table**:
- **Gaitem table @ `7FF46F6AA400`** (entry i = base + i*0x10), 16-byte entries `{handle u32 = 0x08000000|serial, gaitemId u32 @+4, count u32 @+8, ordinal u32 @+C}`; empty entry = `FFFFFFFF 00000000 0 FFFFFFFF`. Entries 12–16 have ids 0x40000019 / 0x40C6B13C.. (category nibble 4 = goods: logs/emblems/OS), ordinals out of order.
  Fresh save: entries 0–11 = starter loadout (BU-TT/A, TURNER-R, P20MLT-04-R, 2× "Back Rail - Weapon Loader" 0x49124/0x4905C,
  FINDER EYE, ORBITER, TOOL ARM, CRAWLER, BST-G1/P10, JOSO, FCS-G1/P01), 12–16 = non-equipment gaitems, **17 = 04-101 MIND
  ALPHA arms (0x131A3850), granted by mission 6 "Attaque du complexe du barrage" and appended at the end** (serial 0x17E).
- **Compact owned list @ `7FF46F6B2020`**, stride 8, **slot i mirrors gaitem-table entry i** (goods slots 12–16 hold the 0x40xxxxxx ids): MIND ALPHA = entry 17 → `2020 + 17*8 = 20A8` ✓. MIND ALPHA's handle is `0x180001A5` (category nibble 1 | serial 0x1A5); starters have `0x0800016D..`; goods entries have handle == id.
  This is what the shop's ownership filter must consult (the buy list hides owned parts).
- Grant did not touch event flags 3xxx/6xxx; "Pièces obtenues" is shown at the result screen, the insert happens on/after OK.
- Next grant = serial 0x17F → table entry 18 @ `7FF46F6AA524`, list slot @ `7FF46F6B20B0` (write-watched in step17 to get
  the add-gaitem backtrace). Emblems are gaitems too ("Emblème acquis" dialog after mission 6).
Mod impact: "mission gives an item" = append a gaitem entry (find the add function via the step17 backtrace) — or call the
game's own grant routine; inventory cap check = count entries.
- **Timing (verified):** the part grant is inserted into the gaitem table when the mission RESULT screen is built — MIND
  ALPHA was already entry 17 while the "Pièces obtenues" dialog was still showing, and pressing OK on it (and on the
  following "Emblème acquis" dialog) wrote nothing to entry 18 / the list slot (write-watches silent). The dialogs are
  display only. Result-screen close sets the mission's 34xx flag (3406 for mission 6) + a **6401** flag (not a lineup
  release flag; emblem/Loghunt-related) + 5011, then 5001.
- **Mission 7 (04:45):** result 3407 → **6210 → 1** (second parts wave = the in-game "new weapons in the shop"
  notification) → 3408/5011. Emblem acquired = **6605** (66xx block = emblems, cleared at new game; mission 6's emblem
  was 6401? — 64xx/66xx are emblem/misc unlocks, not lineup flags). Dialogue 3920 pulses. No part grant this mission.
  Shop waves observed so far on a fresh save: 6200 (mission 2, with Market 3127), 6210 (mission 7).
- **Add-gaitem call chain (captured on a shop PURCHASE of VP-60LCS-L 0x0219DF50 = 35250000, 04:47:45):**
  table entry write @ `exe+C2991F` (R8 = entry index 18) ← `C294DC` ← `B866B4` ← `C2A1F5` ← `474EA7` ← **`B86874`**
  ← **`473F79`** ← **`46C0E3`** ← `1B9CCB7` ← `174E33D` ← `15FF2E5` ← `D30716` ← `D30870` (purchase menu) …;
  compact-list write @ `exe+C2A0B2` ← `474EDD` ← `B86874` ← (same chain). `B86874` = CSGaitemAccessor layer (same class
  as `+B885CF` GetPrice); `46C0E3` / `473F79` = GameData "give item" layer — the callee at one of these is the API to
  call from the DLL to grant a part. Emblems are NOT in the gaitem table (separate store). Purchases and mission grants
  both append at the next free entry.
- **Inventory manager = `GameData+0x600`** (`7FF46F6BC520`): `+0x10` and `+0x30` → gaitem table `7FF46F6AA400`, `+0x38/+0x48`
  → sub-containers, `+0x08` = `0x7C000000800` (capacity/limits?). **AddItem core = `exe+474D80`** (allocs table entry via
  `C2A180`/`C2A0F0` → write `C2991F`; list write via `C29F10` → `C2A0B2`). Callers of the core: **`exe+473F74`** (inside the
  purchase wrapper `exe+473EE0`: `push rdi; sub rsp,40; ebx=r9d(qty); rdi=rcx(mgr); B8A230(&l34); B86700([exe+4E14008],
  &desc, &l34); 474D80(rdi, &desc, ebx, r9d=[arg5+0], [+20]=[arg5+4], [+28]=byte arg7)` — arg5 = ptr to `{gaitemId, x}`),
  **`exe+46B2E5`** and **`exe+475A94`** (other entries — candidate mission-reward / script grant paths; to be confirmed by
  the entry-19 watch on the next Loghunt/mission part reward).
- **Reward/script grant path candidate = the function around `exe+46B2E5`:** `474060(rbx+0x3D0, &id)` (id-set lookup, same
  routine the NEW-badge predicate uses on GameData+0x230) → if found: `[rbx+0xBCC] = 2` ("already have") and skip; else
  `474D80(rcx=rbx+0x3D0, rdx=r12 desc, r8d=r13d, r9d=B879A0(&desc) id, [+20]=0, [+28]=byte)`. Since mgr = GameData+0x600,
  `rbx = GameData+0x230` → the player-items object spans GameData+0x230.. with the table container at its +0x3D0 and a
  status dword at +0xBCC. "Add unless already granted" = mission/script reward semantics (MIND ALPHA path). Purchase
  wrapper `473EE0` callers: 46C0DE / 46C23E / 46C2DD (shop). `475A94` = a third entry (stack/merge variant, goods?).

## CONCLUSION — how the shop gets updated (verified on a fresh save, 2026-09-21)
The shop has **no stored contents**. Every frame while it is open, the builder (`exe+E88F00`) walks the lineup rows and
keeps a row iff the release predicate `exe+159ACB0` passes (bypass map → rewardCondition==0 → flag≠0 → `IsEventFlag(flag)`),
then the ownership filter drops parts already in the gaitem inventory. **"Updated after a mission" = the mission-end event
script calls `SetEventFlag(wave)`** (6200 with Market 3127 after mission 2; 6210 after mission 7) and the per-frame
predicate sees the new flag immediately — no param reload, no list rebuild trigger, nothing cached to invalidate.
So replacing the contents = deciding, at mission end, which rows pass the predicate. Two levers, both copy-independent:
1. **Flags** (vanilla's own lever): `SetEventFlag(exe+B08500)` works from outside. Limitation: shipped rows share wave
   flags (30 rows on 6200), so per-item control needs per-row unique flag ids written into the LIVE lineup param
   (copies migrate on Continue/exit-to-title — the earlier title-screen patch hit an orphan).
2. **Predicate call-site hook at `exe+E89074`** (`call 159ACB0`, itemCtx in rcx, row at `[itemCtx+8]`, result in al):
   detour to our function → return 1 only for the chosen rows. Scoped to the shop builder (the other 3 predicate callers
   incl. the lineup ctor are untouched), immune to param-copy migration, ownership filter left as-is (on a run save the
   chosen parts are unowned). **Recommended.**
Mission-end trigger (poll CSEventFlagMan, copy-independent): result screen = 3139/3200 → 1 … → 0, per-mission 34xx → 1,
then 5011, then 5001. Item granting stays vanilla (out of scope) — see the inventory section only for reference.
- **LEVER 2 PROVEN LIVE (2026-09-21 04:58, fresh save, CE auto-assembler `tools/ce/hook_shop_predicate.cea`):** patching
  the 5-byte `call` at `exe+E89074` to a cave that returns 1 only for a chosen table of row `+0` equip values made the
  shop show **exactly** RANSETSU-AR / TIAN-LAO arms / KASUAR-44Z head (all from UNRELEASED wave 6230) and nothing else —
  released waves hidden, no crash, no focus issues (no breakpoints). Cave allocated near the module (`0x13FFF0000`) so
  the rel32 call reaches. Side benefit: the bypass map check lives inside the replaced predicate, so the VE-60SNA
  "can't hide it with flags" problem disappears with this lever. Ownership filter untouched (buying a chosen part
  hides it naturally). Disable = restore `call exe+159ACB0` (DISABLE section).
- **Live RANDOM REROLL test (05:08):** overwrote the cave's chosen table while the shop was open (new random picks
  GU-Q1-L / BD-011 MELANDER core / arms 52000100). **Per-frame SOURCE list reflected it immediately** (one-shot trap at
  exe+E87FDE: count=3, ids 130A32C0,13197564,00E6DD90 = the new set), but the on-screen **visible container (vtable
  exe+395CE18) kept the old entry** until a UI refresh — it is built on shop open / tab change, not re-filtered per frame.
  Design consequence: reroll the chosen table while the shop is CLOSED (mission end / garage return) — which is the plan —
  and never rely on an in-shop live swap. Chosen table holds row `+0` values (0x030A32C0 style); source entries carry the
  gaitem form (0x130A32C0).
- **GiveItem (reward-path) signature — from its 12 callers (452D98, 46F59A, 47FD37, 480053…480D99):**
  `desc = exe+B86700(rcx=[exe+4E14008], rdx=&desc_out, r8=&itemId)` then
  **`exe+46B1C0(rcx=GameData+0x230 (player-items obj), rdx=desc, r8d=qty, r9b=1, [rsp+20]=byte, [rsp+28]=byte, [rsp+30]=byte)`**
  (reward callers pass 1,1,1). Inside: `[obj+0xBCC]=0`; extracts id; `474060(obj+0x3D0,&id)` dup check → status 2;
  else `474D80` add → then `46FCB0` + callback `[obj+0x570]` (notification). Real entry is `46B1C0` (the earlier
  "46B1BD" was two padding bytes). Prototype caller: `tools/ce/giveitem_cave.cea` (`giveitem(rcx=id)`).
- **Mission-end trigger test (mission 8, 05:46):** flag **3139 1→0** fired exactly at result-screen close and the
  watcher rerolled — but the prototype took **46 s** (full-memory param scan per reroll), so the table changed after
  the player had already opened the shop, and the visible list doesn't refresh per frame → "same shop". Trigger OK,
  latency bug. Fix: cache the param/pool, reroll in ms. Rule for the DLL: the reroll must complete before the garage
  UI is reachable (do it synchronously at the trigger; it is just a few writes). Mission 8 also set wave 6220 (vanilla).

## RANDOM UNLOCK PROVEN (2026-09-21 06:04, fresh save)
Calling the game's own reward-path grant from a cave (`tools/ce/giveitem_cave.cea`: `desc = B86700([exe+4E14008], &out,
&id)`; `46B1C0(GameData+0x230, desc, 1, r9b=1, 1,1,1)`) with a random UNOWNED weapon (WR-0555 ATTACHE-L, 15020300 =
0x00E5310C) returned 0x59 and **appended it to the gaitem table (entry 25, handle 0x080001D0, count 1) + compact list** —
no crash, executed via CE `executeCodeEx` from a foreign thread while idle in the garage. Item giving therefore stays
100 % vanilla: the mod just picks the id. (Mission-9 in-mission log pickups go through `exe+4757B0` (goods variant) from
`B04301`; the reward path is `46B1C0`.) Instant reroll trigger also confirmed on mission 9: `3139 1→0` → reroll in 0.00 s
→ shop showed the new set. Mission 9 also set 3130/3131 (OS Update menu), emblem 6607, result 3451.
- **Frame-part grant (06:05):** LG-011 MELANDER legs in the PREFIXED gaitem form (`0x10000000 | row+0` = 0x1328B740)
  → rax 0x5A, entry 26 handle `0x180001D1` (category nibble 1 in the handle, like MIND ALPHA's 0x180001A5). Id rule for
  GiveItem: weapons = row+0 as-is; frames/internals = `0x1<<28 | (row+0 & 0x0FFFFFFF)` (matches the gaitem table ids).
  Next: grant at mission end (5074 1→0, ~4 s before 3139 rises) to see whether the vanilla "Pièces obtenues" dialog
  lists the granted part (`tools/reroll_on_mission_end.py` now does GRANT on 5074↓ and REROLL on 3139↓).
- **Mission 10 (06:18): mission-end GRANT works but is SILENT.** Watcher granted KRANICH/60Z-R (0x01CD6D00) at 5074↓
  (06:18:20, 3 s before the result screen 3139↑); it landed as entry 27 and shows in Assembly; instant reroll at 3139↓
  OK. **No "Pièces obtenues" dialog.** The player-items obj callback slot `[+0x570]` is NON-null in the garage
  (7FF4D2C32400), so the dialog is not gated by it — the result-screen "parts obtained" list must be built by the vanilla
  reward flow (mission reward data / script), not from grants. → Capture the vanilla MIND ALPHA reward path on a fresh
  save (profile `ac6-roguelike-fresh2.me3`, savefile `AC6_roguelike_fresh2.sl2`) with the self-re-arming inventory watch
  (`tools/ce/step19_watch_inventory_dynamic.lua`) armed before mission 6. Note 06:21:17/34: a save reload flips every
  progress flag 0→1 (all 34xx/62xx/64xx/66xx/5011) — a reload signature, distinct from mission-end.
  Handles get renumbered on reload (entry 25 0x080001D0 → 0x080001FD).

## VANILLA REWARD PATH CAPTURED (fresh2 save, mission 6 "Attaque du complexe du barrage", 2026-09-21 07:03)
Self-re-arming inventory watch (`tools/ce/step19_watch_inventory_dynamic.lua`; note CE `readInteger` is unsigned →
compare to 0xFFFFFFFF) on the next free gaitem entry. **Timeline:** "Pièces obtenues" dialog is shown BEFORE the
results screen (mission-end script), the table still has no MIND ALPHA while it is open; **pressing OK inserts it**
(07:03:17, entry 16, handle 0x18000038) and 2 s later a goods item 0x40000019 (count 3); only then 5074→0 (07:03:19)
and the results screen 3139→1 (07:03:23).
- **MIND ALPHA insert chain:** `C2991F ← C294DC ← C2A1F5 ← 4757B0 ← B04301 ← 1A9F7A7 ← 114CE26 ← 1A9F7A7 ← 19FED6B ←
  1B22A9F ← C8857A ← C8691A ← FD0EC7 ← C8648D ← D62CDD ← D5B52B ← A53B88` — i.e. the **script VM's give-item
  instruction** (`B04301` handler → `4757B0`, the *goods-style* AddItem entry, the same one log pickups use), NOT the
  `46B1C0` reward-style function (that one is used by other givers; my test grants through it worked but silently).
- **0x40000019 insert chain:** purchase wrapper `473F79 ← 46C243 ← 1544705 ← B2B50E ← 46C5CC ← B860E8 ← B8A7BF ←
  46F129 ← 480042 …` (the 480xxx cluster).
- The dialog itself is a separate script step shown before OK (the script waits for OK, then gives). Poller lesson: the
  CE-free flag poller caches page addresses — restart it after a New Game (pages were re-created; it went stale).
- **Mechanism behind the dialog (decoded):** `exe+B042C0(cl=flag)` = **CommitPendingObtainedItems**: `obj = GameData+0x230`;
  `list = 46E180(obj)`; `4756E0(list, flag, obj)` iterates the pending elements (`[e+0]=id`, `[e+8]`,`[e+9]` bytes → per
  item `C2A180`/`C2A0F0` insert = the 4757B0 site) then `46F660(obj)`. So mission rewards are first queued in a PENDING
  list on the player-items object, the "Pièces obtenues" dialog displays that list, and OK runs the script step that
  calls B042C0 to move them into the inventory. **Mod plan:** push our random part into the pending list before the
  dialog is built (need the add-to-pending function), and vanilla shows + commits it.
- **Pending obtained-items list = vector at `mgr+0x80` (begin) / `mgr+0x88` (end) / `+0x90` (cap)**, mgr = GameData+0x600
  (`46E180(obj)` just returns `obj+0x3D0` = mgr). `474420(mgr,&range)` walks it; commit `4756E0` inserts each element
  (`{id u32 @0, byte @8, byte @9}`) via `C2A180`. Accessor callers AF4237/AF43D3 are lookups (`473E50`), AF6220 is a
  commit variant (`4756E0` with dl=0) — the ADD function is still unknown → `tools/ce/step20_watch_pending_end.lua`
  (write-watch on the end pointer) will capture it at the next "Pièces obtenues".
- **Open: how is the "Pièces obtenues" dialog opened, and does its script step run on every mission end?**
  If the step is universal (dialog skipped when the pending list is empty) → appending to the pending list is enough.
  Otherwise we need the menu-open function. Instruments (`tools/ce/step21_dialog_hunt.lua`): once-trap on the commit core
  `4756E0` (fires at any mission end that runs the step; dl=1 commit / dl=0 clear) + a push-chained short-lived trap on
  the accessor `46E180` (the readers right after a reward is queued = the dialog builder → menu-open function).
  Pending vector element stride = 0x10 (`add rbx,0x10` in the commit loop); capacity is tiny (1 element right after a
  commit) → never push into it by hand, use the game's add function.
- **UNIVERSAL STEP PROVEN (07:19:55, a NO-reward mission end):** the commit core `4756E0` ran anyway (dl=1) from
  `B04301 ← D5C639 ← D45380 ← C8857A ← C8691A ← A53B88 ← C8648D ← D5B52B …` (result flow), 2 s BEFORE 5074↓, then the
  pending vector was cleared (`476FA2 ← 476FD7 ← 475855`). No dialog because the list was empty. ⇒ The mission-end
  "obtained parts" step always runs; the dialog appears only if the pending list is non-empty. **Random unlock plan:**
  append our part to the pending vector before that step (hook the step's entry near `D5C639`, or the last in-mission
  trigger) — vanilla then shows "Pièces obtenues" and commits on OK. Still needed: the game's append function
  (pending-end watch armed to catch it at the next reward).

## Next-session plan + shop cohesion requirements (user, 2026-09-21)
- **Fresh start again next session; make a SAVE BACKUP right before launching mission 6** (copy the run's `.sl2`, e.g.
  `AC6_roguelike_fresh3.sl2` → `.bak.before-m6`) so the MIND ALPHA reward (the append-to-pending capture) can be retried
  again and again by restoring the file (game must be closed to copy/restore reliably).
- **Shop cohesion (design requirements):**
  1. Reroll at mission end (trigger 3139↓ already proven; reroll before the garage is reachable).
  2. Survive quit → load: the same 3 parts must come back. Proposal: no external state — persist a **run seed + reroll
     counter inside event flags** (an unused flag range, packed bitwise via `SetEventFlag`; flags are serialized in the
     .sl2 by the game). Chosen set = deterministic PRNG(seed, counter) over the eligible pool sorted by row id. The
     counter is incremented only after the mission-end step completes.
  3. Owned parts: the predicate hook shows chosen parts, the vanilla ownership filter hides anything already in the
     gaitem inventory → a part bought before quitting stays hidden after loading (it is owned); the slot stays empty
     until the next reroll (Slay-the-Spire style). Also exclude owned parts when PICKING so a fresh roll never offers
     something already owned.
  4. No separate RNG state to persist: derive everything from (seed, counter).

## 2026-09-24 — Pending "obtained parts" list: append function FOUND STATICALLY (no live capture needed)

Item manager `mgr = GameData+0x600` (ctor near exe+4735xx zero-inits +0x98/+0x99/+0x9C; dtor exe+4738E0 erases the vector).
- `mgr+0x78` = `vector<u32>` `{alloc@0, begin@8, end@0x10, cap@0x18}` of 4-byte **item ids** (same value the item
  descriptor carries: weapons row+0, frames 0x10000000|row).
- **`exe+477170` = `vector<u32>::push_back(vec=rcx, const u32* id=rdx)`** (grow via exe+476A40 when end==cap, copy via
  exe+B9C8E0, then `add [rcx+0x10],4`). `exe+476FB0` = erase(first,last) (used by commit core exe+47583C and the dtor).
- **`mgr+0x99` = DEFER byte.** In AddItem core `exe+474D80(mgr, desc, qty, r9, a5, a6)` at 474DBB: if `a6==0 && mgr+0x99!=0`
  → `B9C990(&tmp, desc)`; `477170(mgr+0x78, &tmp)`; `B9CA30(&tmp)`; **return -1** (item queued, not inserted).
  Commit core exe+4756E0 sets `mgr+0x99 = 0` (475700) then iterates the range getter exe+474420 (expands each id to
  {id, byte@8, byte@9} via B9C720 type==4 check → exe+477090 push16) and inserts via C2A180/C2A0F0.
- So the VANILLA reward mechanism is simply: the mission's script give-item runs while the defer byte is 1 → queued →
  the universal mission-end step shows "Pièces obtenues" and commits on OK. My earlier silent test grants ran in the
  garage (defer=0) → inserted directly. **Mod plan: grant the random part while defer==1 (during the mission, e.g. on
  5074↑ or at 5074==1 before mission end) via the same give path → vanilla dialog + commit for free.** Alternative:
  call 477170(mgr+0x78, &id) directly at mission end before the step.
- Writers of `+0x99 = 1` found statically: exe+6FA520 (2-instr setter, callers 7156AB/71E03D/7214C7/727D19 on `[x+0xC8]`
  objects — probably another class), ACA3AB, 12FFEBA, 1D969E4, 1D98552 — which one targets mgr: to be confirmed live with
  `tools/pending_monitor.py` (polls defer byte, pending ids, flag 5074).
- **LIVE (2026-09-24, fresh3, mission 1 in progress):** `mgr+0x99` became 1 right after New Game (mission active).
  The give wrapper `exe+46B1C0(obj=GameData+0x230, desc, qty, r9b, a5, a6, a7)` passes **a7 as 474D80's 6th arg =
  "bypass defer"**: with a7=1 (old cave) the grant inserted directly (rax=77 = slot). New cave
  `tools/ce/giveitem_defer_cave.cea` (a7=0): grant of 10050100 during the mission → **rax=-1 and the pending vector got 1
  element `08000013`** = a gaitem HANDLE (0x08000000|serial), i.e. B9C990 allocates the gaitem entry and the pending
  list holds handles; commit on OK inserts them into the inventory lists. Awaiting the mission-end dialog to confirm.
- **PROVEN (2026-09-24 ~03:55, mission 1 "Entrée illégale"):** the deferred grant produced the vanilla **"Pièces
  obtenues" dialog (SG-026 HALDEMAN)** before the results; OK → commit core: `mgr+0x99` 1→0, pending vector emptied.
  During the mission the pending list also accumulated 4 handles of the form `40C6Bxxx` (mission pickups / goods) that the
  dialog did not show (getter filters type==4 = parts). Flag 5074 rose only at the results screen on this run, so the
  **defer byte `mgr+0x99` is the better "mission active" signal**: 0→1 = mission start (grant the reward, deferred),
  1→0 = commit at mission end (reroll the shop while it is closed).
  => Mod recipe: on defer 0→1 call `46B1C0(GameData+0x230, desc(B86700), 1, r9b=1, a5=1, a6=1, **a7=0**)`.

## 2026-09-24 — DLL v1 WORKING END TO END (fresh3 save, missions 1-8)

Proven live with `dll/src/lib.rs` (built 05:35): reward queued at sortie → re-queued after the game's own clear at
the real mission start → vanilla "Pièces obtenues" dialog → OK commits → reroll on flag 5074 falling → 3 items in
the Market (1 weapon + 2 parts) → bought parts vanish (vanilla ownership filter) → quit+Continue restores the same
state from the save (event flags 7800-7827, direct bit-page writes + dirty byte man+0x60).

Hard-won facts:
- **Dialog content is built at the mission-clear instant** (5074↑). A grant made after that is committed silently
  (mission 7: queued 48 s before the commit, no dialog). Grant at mission START (defer 0→1 = sortie) and re-queue
  whenever the handle vanishes from the pending list (the game EMPTIES the list at the real mission start, ~2 min
  after sortie, and on restart-from-pause; abort→garage empties it too, so no double reward is possible).
- **The give wrapper blocks ~25 s** when called during loading/transitions (waits for a game lock; returns when the
  game is idle). Calling game functions at the commit instant froze the game once (mission 6, black screen, 2 cores
  spinning) → the DLL now calls NO game function except the give at sortie/re-queue.
- **Win64 ABI trap:** narrow (u8) args are not zero-extended; `SetEventFlag` reads `r8d` (`mov r15d, r8d`), so a
  `u8 state=0` never cleared (only "extra 1 bits" mismatches). All bool args are now u32; flags are accessed
  directly anyway: CSEventFlagMan {div@+0x1C=1000, bytes/block@+0x20=125, pages@+0x28, map hdr@+0x38 (nodes
  {left@0,right@0x10,nil@0x19,block@0x20,type@0x28,idx|ptr@0x30}), dirty@+0x60}; bit = MSB-first in byte.
- **Flag ranges:** 99xx are GAME-OWNED (9901/9908/9921 set at mission end); 8800 was cleared by the game at a
  mission start; blocks 4 and 7 (probes 4000/4500/4900/7000/7500/7900) untouched over 3 missions + reloads →
  state lives at 7800-7827 (3×9-bit lineup row indices + valid bit 7827).
- **Item id tags by lineup equipType** (from the gaitem table): 0 weapon = bare id; 1 frame (50-53xx) 0x10000000;
  4 booster (60xx) 0x60000000; 5 FCS (70xx) 0x70000000; 6 generator (65xx) 0x50000000. A wrong tag → nameless
  "(60020000)" dialog entry or an orphan table row (hidden everywhere). Pending list holds gaitem HANDLES
  (weapons 0x08…, frames 0x18…, goods 0x40…). Give-path error byte 4 accompanies a successful queue (rax=-1).
- VirtualQuery-heavy scans (2048/entry) stall for tens of seconds during loads → one query per page + cached
  flag page (staged build in dll/target2).
- Loop timestamps in the DLL log are GetTickCount64/1000; pending_monitor/flag_monitor give wall clock.

Next: economy (prices ×N, COAM reward zeroing, Replay Mission 3134 off), inventory cap, death tax, rarity pools
(user: organize the pool by rarity so kinds/tags are explicit per record).

## 2026-10-26 — Economy, escrow, penalty: IMPLEMENTED + VERIFIED (DLL, fresh3 save)

**Param repository** (the live params, stable across title/Continue): `[exe+4E27300]`; slot i at `+0x80+i*0x48`
`{u32 count, ptr res}`; file = `[[res+0x80]+0x80]` (standard PARAM: rowCount u16 @+0xA, row headers @+0x40 stride
0x18 `{id, pad, dataOffset u64}`); name = wide string at `res+0x18`. Indexes: EquipmentLineupParam 0x29,
MissionParam 0x91 (row 0x160: currencyReward +0x2C, event bonuses +0x5C stride 8 ×5, enemy bonuses +0xDC ×8),
ArenaParam 0xC8 (initial +0x20, repeat +0x30). The DLL zeroes positive pay/bonuses (219 fields) + arena repeat.
Scanned "EQUIPMENT_LINEUP_PARAM" copies are stale/half-built — always go through the repository.
**COAM** = PlayerGameData+0x74 (i32; +0x78 lifetime). AddCoam `exe+48DD70(gd, delta)` clamps to [0, 999999999]; the HUD
calls it ~100×/s with 0. Detoured (first 8 bytes) to record the UNCLAMPED result → negative-balance detection.
Verified: briefing pay 0; a clear cost exactly the ammo+repair shown (−33,268).
**Replay Mission** locked by clearing flag 3134 every tick.
**Player AP**: WorldChrMan global `exe+4E0F598`; AP = `[[[[g]+A5A0]+320]+0]+278` (max +27C).
**Saves:** the game never saves during a mission (requests stay pending). Save request = OR bit 0 into
`[[exe+4E140B0]+0xCB4]` (what `RequestSave()` exe+B90140 does; per-frame save step exe+1272440 honors it).
**Escrow:** flag 7828 set + save requested when flag 5003 rises (pre-mission talk; game still saves then), cleared at
mission clear. Garage-open events: save load = flag 1000 0→1 (ignore 5003 on that tick), or mission end without clear
(only if flag 1000 still set: shutdown flips the mission byte too). Verified: Alt-F4 mid-mission → penalty on load.
**Game-thread work:** detour of the per-frame `call exe+1272440` at exe+1274418 → run queued tasks after it.
**Inventory:** index i < [mgr+1C] → [mgr+40]+16i, else [mgr+30]+16(i−[mgr+1C]); count [mgr+08]. Entry {handle, id,
count, ordinal}. `RemoveItemAt exe+475150(mgr, index, 1)` = what selling calls (captured via write-watch).
**Assembly = THREE parallel arrays in PlayerGameData, slot k:** inventory INDEX +0x24C+4k, HANDLE +0x424+4k, ID
+0x4F4+4k. Slots: 0 L-arm, 1 R-arm, 2 L-back, 3 R-back, 6/7 back rails, 18 head, 19 core, 20 arms, 21 legs,
23 booster, 24 generator, 25 FCS. The Assembly menu resolves slots through the INDEX array (read live); a stale index
shows "(NÉANT)". Handles are reassigned at every load (indexes are saved). Starters (plain ids, owned from New Game):
50070400/51070400/52070400/53070400, booster 60070400, generator 65000000, FCS 70000000. Starter prices = cheapest
shop part of the type: 58k/158k/79k/139k/53k/90k/67k.
**Penalty verified (replayed from a backup):** weapons removed + slots emptied, non-starter frame/internals replaced by
starters (fully named in Assembly), starter slots charged (350,000), escrow cleared, save requested.
**Auto-sell verified:** −247,146 covered by selling the 2 most expensive stash parts (180k + 127k) → 59,854.
**Game over detection verified:** −584,146 with an empty stash → "GAME OVER" (balance floored to 0).
**Pause menu blocked (verified):** game step tests the pause input (`call exe+1195100`) then `call exe+D834D0` at
exe+127733D sets the menu request `[[[exe+4E15530]+18]+8]+0xD0 = 7` → "MainMenu" (the in-mission pause menu; variants
_ForNpcArena/_ForOnlineArena/_ForAcTest/_ForTraining). The DLL hooks that call and drops it while the defer byte is 1.
**Death → garage (verified):** the mission EVENT SCRIPT opens the death menu ("MissionRetryMenu") via
`call exe+DC1E40` (edx=1) at exe+17A244F. Quit runs exe+D56DD0 → **RetireMission() exe+B002A0** (no args: discard
pending items via 4756E0(dl=0), restore pre-mission state, `[[exe+4E29CC0]+0x3FAE]=1`, tail-call exe+B059A0(0) =
return to garage). The DLL hooks the script call (menu built as usual) and calls B002A0 on the game thread next frame;
the death menu flashes for one frame. Death → retire → failed-mission penalty all chain correctly.
TODO: loss dialog (list lost/sold parts), game over = save reset, hide the one-frame death menu flash.
**Game over (implemented, UNTESTED at end of 2026-10-26 session):** run-over flag 7829 set + save; then return to title
like the menu does (exe+1285A00): `[step+0xBC] = exe+B8ECF0(); setGameStep(step, 4)`. setGameStep exe+1285200 starts
with `jmp exe+6DBC03` → hooked to capture the main step object (captured at boot on the title transition). A save
loaded with flag 7829 goes straight back to the title.
**Messages (next session):** text = category 204 etc. via MsgRepository [exe+4E256D8], LookupEntry exe+323FE20(repo,
0, category, id) → replaceable whole (short function). Dialog D68430(out, ownerMenu, textObj) needs a menu owner →
instead capture the event-script "generic dialog" used by garage announcements (category 74, e.g. 2100010 "De
nouvelles pièces sont disponibles") by restoring fresh3 `.before_mission6` and playing to the mission 7 announcement.

## 2026-10-26 (later) — Return to title SOLVED (one byte), game-over action VERIFIED on a dead save

How the game navigates (title is NOT a separate level; no loading screen):
- **TitleStep** (vtable exe+3A62D80, handler table at exe+4E1A710 with names): 0 InitMenu, 1 BeginLogo, 2 BeginTitle,
  3 BeginNewGame, 4 PlayGame, 5 GameStepWait, 6 EndFlow, 7 EndFlowWait, 8 NextLap, 9 MenuJobWait (title menu), 10 Finish.
  State: cur +0x48, next +0x4C; setGameStep exe+1285200 writes +0x4C (body at exe+6DBC03). Destination area +0xBC.
  Continue = wrapper exe+1285A00: +0xBC = area, step 4 (PlayGame) → 5 (GameStepWait). Forcing step 4 from outside =
  "play again" = garage reload / half-torn garage (what all earlier attempts did).
- GameStepWait waits for `[ [TitleStep+0x2D0] + 0xD0 ] == 0` (child **InGameStep**, vtable exe+3A56450, table at exe+4E19558:
  Initialize, InitWait, RequestWait, Finalize, MoveMap_*; in the garage it sits in MoveMap_Update (7) holding a
  **MoveMapStep** job at +0xE0 (vtable exe+3A5CFA0; states ... 17 STEP_MoveMap, 18 Cleanup, 19 Finish; done when +0x48 == -1).
- The in-game update exe+1271A30 (called from exe+1274423, same function as our frame hook) ORs eight "leave map"
  conditions; when set, a byte state at [this+0x168] runs 0→1→2 (fade) →3→4 (exit sequence, MoveMapStep → Cleanup).
  After the map exits, the next area comes from `[[exe+4E140B0]+0x14]` (or +0xC1C in one mode): -1 → title.
- **RETURN TO TITLE = `byte [[[exe+4E15530]+8]+0x173] = 1`** (menu-manager "leave map" request; the update copies the
  OR result to +0x174). With destination -1 (garage default) the game exits to the real title. Plain write, any thread.
  Verified: from an idle garage, and as the DLL's game-over action on a run-over save (Continue → 5 s → title).
- Dead ends documented: forcing setGameStep(4) (= PlayGame), hooking the main step's menu update (exe+1284F31 is not
  called in the garage), the generic menu activation exe+D623B0 (same 17 calls for opening and confirming a dialog).
- Tools: `tools/ce/tracer.lua` (CE single-step call tracer: logs calls with args, steps over them, follows obfuscated
  jumps); Tab opens the system menu (synthetic Escape doesn't register; mouse clicks need a held press).

## Game over at death: VERIFIED (2026-10-26)
- Death (AP 0) with `project_failure` < 0 -> msg override shows "GAME OVER" in the vanilla death notice (small red
  top-left label, cat 200 id 254000), then death-menu hook -> leave-map byte -> title (~14 s after death).
- The run-over flag set in mission is NOT persisted (game never saves in missions). Not needed: the escrow flag was
  saved at the pre-mission talk, so Continue -> garage -> penalty + auto-sell reproduce the same deficit -> GAME OVER
  -> flag 7829 saved -> title in 5 s. Same path covers Alt-F4 at the death screen. Next Continue -> straight to title.
- Cosmetic: on that Continue the garage is visible ~8 s before the bounce.
- UPDATE (same day): dead save detected AT LOAD (flag 1000 edge): run-over flag OR (escrow set AND
  project_failure < 0; the inventory/COAM are already loaded at that edge, projection matched -337000). The DLL
  raises the leave-map byte at once and re-raises it every second until the save unloads (the first request,
  made during the load, is dropped; the 1 s retry lands). Continue -> back to "press any button" in ~4 s, garage
  never shown, no penalty applied, nothing saved (the unflagged escrow save is dead every time it is loaded).

## Failure plan (2026-10-26)
- `penalty::FailurePlan` is built once when the sortie is committed (deployment talk, same moment as the escrow
  save) and rebuilt from the save on load when escrow is set (identical: the loadout and inventory cannot change in a
  mission). It lists weapons lost, frame slots swapped to starters (using a spare starter from the stash when owned),
  starters to pay, and the sellable stash (by inventory HANDLE, most expensive first, spare starters excluded).
  `balance(coam)` drives the death check and the load check; `apply_penalty` executes the same plan.
- Verified: talk plan 59854 -> death -> retire -> penalty applied -> balance 59854. Not yet exercised live: weapon
  loss / frame swap / spare-starter branches of the plan (this save is all starters).

## Standalone dialog + loss report: VERIFIED (2026-10-26 night)
- Talk-script (EzState) command dispatcher: exe+179F7B0-ish, `switch(cmd)` over 0..0x103 via byte table exe+17A32AC /
  dword table exe+17A30A4; args object vtable: +0x10 command id, +0x18 count (= args+1), +0x20 get(i) -> {value, _, kind@+8
  (2 int, 1 float)}. Case 225 = MissionRetryMenu (exe+DC1E40), case 17 = 5-arg generic dialog (wrapper exe+1783470).
- **Generic dialog job**: exe+DC1650(&job[3], args) wraps args in a std::function and calls the menu manager
  ([exe+4E15530]+0x18, D74BC0) which invokes it at once: exe+DBE570 reads args 1..5, needs global [exe+4F348C8]
  (exists in the garage, null at the title -> fatal error path), byte +0x5342 == 0xFF in the garage -> exe+D67470(out,
  owner supplied by the framework, arg1, params). Then exe+DC0BC0(&{vtable exe+386E0C8, job[1], job[2]}) opens the job;
  exe+DBA200 only resets a handle's vtable (no refcount). Args 1 1 0 1 + msgId = info dialog with one OK button; the
  text is looked up in category 30 -> our msg hook serves custom UTF-16 text for id 990000001.
- The DLL builds a fake args object (own vtable, `static mut` - an immutable static crashed the game) and opens the
  dialog from the garage game-thread hook. Box does not grow past ~12 lines; long lines wrap -> one line per category.
- Part names: categories 11 weapon, 12 frame, 38 booster, 35 generator, 41 FCS, keyed by bare equipId (`tools/fmg.py`).
- Dead ends: exe+D68430/D67220 (text / yes-no dialogs) take a menu "result" object only valid inside a menu update;
  "newSaleParts"/"newOwnParts" strings are menu badge conditions, not announcement dialogs.
- **Verified live**: gear save (HI-18 L-arm, LAMMERGEIER head, TALBOT FCS bought; starter head/FCS as spares), plan at the
  talk: 1 weapon, 2 swaps, 5 starters 519000, spares excluded from sale, balance 81000 -> death -> retire -> penalty
  (spares equipped, no re-grant) -> dialog "ÉCHEC DE LA MISSION : PERTES" with names, cost, balance 81 000 -> Assembly
  shows empty weapons, FINDER EYE, SCT-G1/P01, AP 9080. Not yet seen live: the "Vendues" line (no stash in this test).
- Debug: write text to %USERPROFILE%\ac6_dialog_test.txt -> shown in the garage dialog.

## Config, prices, sell hook (2026-10-27)
- `ac6_roguelike.toml` next to the DLL (written with defaults when missing; tiny TOML subset, BOM tolerated):
  [economy] mission_pay_percent, arena_repeat_pay_percent; [shop] part_sell_percent, weapon_sell_percent,
  item_charge_percent; [prices] `equipId = value` or `"part name" = value` (name key = alphanumerics, upper case;
  a name matches both side copies of a weapon: 10xxxxxx right arm / 15xxxxxx left arm, 30xxxxxx / 35xxxxxx back).
- Value of a part = game shop price; 0-priced parts (starters, MELANDER C3, JAILBREAK) valued by [prices] (defaults
  from tools/price_model), else the cheapest shop part of the type. Core expansions (75xxxxxx) never sold.
  Shop buy price = value x item_charge (lineup price rewritten at boot), failure charge per starter slot = same,
  sell = value x part/weapon_sell (auto-sell + Vente menu). Left/right copies always share a price (79 pairs checked).
- **Sell price hook**: exe+B885D0(desc) = sale value (item id at desc+8, reads the lineup price via exe+435B50); the buy
  list never calls it. Call sites hooked: exe+D2FDDA (Vente list), exe+D303DD (sale / confirmation), exe+474598 (loop).
  Verified: TURNER listed and sold at 13 750 (25% of 55 000), credits +13 750.
- Price model (tools/price_model): wiki stats (Fandom frames/internals, Fextralife weapons, manual_stats.json) joined
  with live lineup prices; per type ridge on log price + progression tier (unlock flag 62xx waves) + melee flag,
  LOO-calibrated, anchored to the 3 cheapest shop parts of the type. LOO error 15-50%: prices follow progression
  more than stats; values are starting points. Unlock flags: 1000 = the 10 starter parts, 6200..6280 = shop waves.
- Tools: tools/stash.py (stash + prices), tools/inventory.py, tools/fmg.py (message tables), tools/rtti.py,
  tools/focus_game.ps1 (click the game to take focus from TextInputHost), held Escape = menu back.

## "Session not ended properly" warning: FIXED (2026-10-27)
- Title warning = message 400003 (cat 203). Session state: byte [[exe+4E140B0]+0xCB8]. OpenSession() exe+B90090
  (=1; one caller, the in-game update at exe+1271354, at every map load incl. mission -> garage); the system menu's
  EndSession() exe+B900AF (=0, menu manager call, +0xCD4); RequestSave() exe+B90140 (mask |= 1 when allowed);
  RequestSave+system exe+B90180 (mask |= 0x200, then |= 1). The title checks the SAVE FILE (not memory) when the
  player presses a button. The mask drops to 0 when a save starts; the file is written afterwards.
- Game-over exit (DLL, all paths: dead save at load, death -> retire -> garage, auto-sell game over): (1) SESSION_EXIT
  set (OpenSession call hooked and skipped), byte cleared, session-end save 0x201 requested (CAS when the mask is
  free); (2) mask clear + 2 s grace; (3) leave-map byte until the save unloads. Death game overs now retire to the
  garage like any death (saves are impossible in missions) and leave from there; the garage only shows as a dark fade.
  Verified: death path and dead-save path from the same session, no warning.

## Rarity (2026-10-27)
**Superseded (2026-10-29) by "Pools in the settings file": named pools instead of tiers 1..3.**
- Tiers 1 common / 2 rare / 3 epic from [part_rarity] (default block generated by tools/price_model/tier_lists.py:
  weapons from a recency-weighted consensus of 4 tier lists, frames and internals from best-parts guide counts);
  every unlisted part is in one pool, [rarity] unlisted_tier (common). Pool split (both side copies counted):
  weapons 128 common / 26 rare / 26 epic, parts 49 / 42 / 4. Shop and rewards: tier by weights (60/30/10), uniform in
  tier. (Lineup row +0x08 = shop unlock flag: 6200..6280 campaign waves, 64xx arena, 1000 starters; not used.)
- Config also: ammo_cost_percent / repair_cost_percent (MissionParam f32 +0x48 bulletCost_rate, +0x50
  repairCost_coefficient), shop_weapons / shop_parts (slots 3.. persisted in flags 7900.., 9 bits each, value idx+1),
  parts_per_mission. tools/paramdef.py computes param field offsets from Smithbox definitions.

## Leaving the map waits for saves (game over without a garage)

`[inGameObj+0x168]` is the leave-map state: 0 → 1 (fade out, timer at +0x298) → 2 → 3 (unload). At exe+12721A0 the
step from 2 to 3 only happens when exe+B8FA50 is true and the save state (B8EE10) is >= 0xD, i.e. no save is pending or
running. So a save requested together with the exit is written behind the black screen, the way the system menu's
"Save and Return to Title" works. A run-ending death retires the mission (death menu never built), and at mission FAILED
the DLL closes the session (cb8 = 0, OpenSession blocked), ORs 0x201 into the save mask and raises the title request
in the same frame: no garage on screen, and the title sees a closed session (no improper-exit warning).

## Save storage, reroll cadence, starting credits (2026-10-27, task 1)

**Event-flag blocks.** CSEventFlagMan holds 640 blocks of 1000 flags (`tools/flag_blocks.py` lists them): blocks 0-9
(flags 0..9999) plus 630 map blocks 2100102.. (ids 21xxxxx), all type 1 (pages in the shared page array). In the
garage of the fresh3 save, blocks 4 and 7 hold only our own bits: old probes 4000/4500/4900/7000/7500 (set in
September, still there = already saved) and the shop state 7800-7829 / 7900+. Block 8 and 9 are game-used (88xx,
99xx). New Game re-creates the pages: blocks 4 and 7 are EMPTY at the first mission start of a new game (verified
twice), so every new run starts with clean state.

**Persistence proof.** Set 4001/4150/4318/4501/4818/4999/7001/7318/7799/7830/7837 (direct bit write + dirty byte, like
the DLL) → save request (bit 0 of the save mask) → file written → game killed → relaunch → Continue → all 11 read back
set. Then cleared again.

**Layout** (offsets.rs): row bitsets = flag base + lineup row index (318 flags each), avoiding the old probes —
`ROWS_SOLD` 4100..4417 (sold since the reroll), `ROWS_LOST` 4550..4867 (lost forever), `ROWS_ROLLED` 7100..7417
(already rolled); `REROLL_COUNTER_BASE` 7830..7837 = missions cleared since the last reroll (8 bits, LSB first). Free
for later: 4435..4549 (4418..4434 = OS_GONE), 4868..4899, 4901..4999, 7418..7499, 7501..7799, 7838..7899, 7945..7999. Helpers on `Game`:
`read_rows` / `write_rows` / `clear_rows` / `put_row` / `has_row`, `get_bits` / `put_bits`.

**Reroll cadence.** `[shop] reroll_interval` (1..255). `on_mission_end`: no shop state (first mission end of a run) or
counter + 1 >= interval → reroll, counter = 0; otherwise counter + 1, shop kept. Rewards are untouched (every
mission). Verified with interval 2 (debug `mission_end`: kept 1/2 → reroll → kept 1/2; counter survives a relaunch)
and on a real clear (new game, first shop rolled, counter 0).

**No shop before the Market unlocks.** The Parts Shop menu opens with flag 3127, set by the game at the mission 2
clear (with wave 6200) — about 30 s AFTER 5074 and after the commit autosave (measured: 5074↑ 99351, commit 99363,
3127↑ 99381). A mission end with 3127 off rolls nothing and does not count toward reroll_interval; the loop rolls the
first shop when 3127 rises (only after such a mission end, never on a save load) and requests a save, since the
autosave already passed. Verified on a new game: mission 1 and 2 clears → first shop at 3127↑, same shop after a
relaunch.

**Starting credits.** `[economy] starting_credits` (100000) = the pay (`currencyReward`) of MissionParam row 2010
(Illegal Entry), the forced first mission of every New Game; every other mission still pays mission_pay_percent (0).
Replay Mission is locked, so each run gets it exactly once and runs already past mission 1 never do. The game shows it
on the results screen and nets the mission's costs from it in ONE AddCoam at the commit (new game, mission 1:
0 -> 78332 = 100000 - 21668, no deficit, shop rolled). Rejected first design (user's idea replaced it): add the
credits from the DLL on the game thread when the first shop is rolled. Added at the commit, they came right AFTER the
game charged the costs on a balance of 0: AddCoam clamps to 0, the COAM detour records the unclamped -27090, the
auto-sell runs on that stale deficit -> reward part sold -> GAME OVER at the first mission end. Lesson: the commit is
where the game charges ammo/repair; money must exist before it.

**Seen on the way (not changed):** the first mission of a New Game starts without the escrow flag (no deployment
talk before it); a death retires the mission and the DLL re-queues the reward just before the abort commit, but the
game drops it (reward not owned afterwards).

## Buy-back: Vente sale event (2026-10-27, task 2)
- **The Vente sale** (live, CE trap on RemoveItemAt during a sale): SellItem **exe+D30A80(listItem, n, flag)** (list
  entry: gaitem id +0x54, handle +0x50) calls **exe+D309D0(&itemId, handle, n, flag) -> count removed** at exe+D30AE2
  (and at exe+D30B59 for a linked item when the first removal yields one). D309D0 clamps n to the owned count and
  calls exe+46B4E0(giveObj, handle, -n, ...) -> exe+46FC7A -> RemoveItemAt exe+475150. D30A80 is reached from
  exe+D30250 (per-item transaction) and from exe+D2F1B0 / D2F2xx / D2F36F (Vente code).
- Static dead end: exe+D2FEB0 (cart commit, cart entries stride 0x20, id +0x10, qty +0x14) also has a RemoveItemAt
  call (exe+D300B8), but a Vente sale never goes through it (hooked: no hit).
- **DLL (buyback.rs)**: call-site hooks at D30AE2 / D30B59 read the item id, call the original, and when it removed
  something map the id (tag | equipId) to its lineup row and set the bit in ROWS_SOLD (4100 + row) and a memory mirror.
  penalty::sell (auto-sell, failure sales) records its sales too; lost parts never pass there. shop_pred lists a row
  when it is CHOSEN or in the mirror (the game hides owned parts); price = the lineup price (value x item_charge).
  The reroll clears the set; each save load (flag 1000 edge) reloads the mirror from the flags.
- **Verified live** (gear_test save): sell RF-024 TURNER (right arm, row 148) and HI-32 BU-TT/A (left arm, row 219)
  -> both logged, both in Achat on their own side at shop price (55 000 / 60 000); buying TURNER back logs no sale and
  hides it (owned); save + kill + relaunch -> "2 part(s) from the save [148, 219]", HI-32 still offered; mission-end
  reroll (debug `mission_end`, same on_mission_end path as a clear) -> list cleared, HI-32 gone.
- **Auto-sales verified live** (gear_test save, COAM set to 600 000 + escrow flag 7828 set, save, Continue): failure
  penalty charged 5 starter slots (621 000), lost HI-18 / LAMMERGEIER / TALBOT, auto-sold HI-32 (15 000) and TURNER
  (13 750) -> 7 750; only rows 219 and 148 entered the list ("sold (auto-sell)"), both back in Achat on their side;
  the lost parts did not. (Writing a negative COAM directly does not trigger the garage auto-sell: the HUD's
  AddCoam(0) reports the balance already clamped to 0.)
## Starter parts always buyable + failure rework (2026-10-27, task 3)

**Shop.** `shop_pred` also lists the 7 starter frame/internal rows (FRAME_SLOTS); the game's ownership filter hides
the owned ones (verified: FINDER EYE sold -> listed at 42 000; CRAWLER/BST-G1/P10 listed after a failure removed
them). They are not in the random pool (`build_pool` runs before `penalty::init` prices their rows). `init` now always
writes a starter row's price (value x item_charge_percent), so a starter is never free even without a [prices] entry.

**Failure plan** (`penalty::FailurePlan`, one deterministic pass, built at sortie / rebuilt from the save on load):
weapons -> all four weapon slots cleared, equipped ones removed + `ROWS_LOST`; every frame slot -> its part removed
(non-starter -> `ROWS_LOST`), filled by `Fill::Spare` (newest owned unequipped part of that slot), `Fill::Buy` (the
starter at its shop price; when the lost part IS the starter it simply stays equipped: lost + bought back) or
`Fill::Empty` (booster under tank legs); then cost; then sales from the stash minus the chosen spares.
- **Acquisition ordinal** = inventory entry `+0xC`: a counter assigned when an item enters the inventory (fresh3:
  New Game loadout 1-12, later parts 51+; a debug grant got 54, the next ones 55, 56...; independent of the entry
  index, which gets reused). Spare = max ordinal, then max value. Not matched against a UI sort (the Assemblage list
  only has "Par défaut" + a cycle key); the counter itself is the acquisition order.
- **Tank legs** = `EquipParamProtector.legsType` (u16 @ row +0xDA) in 300..302 (Paramdex LEGS_TYPE: 0 none, 1 biped,
  100 tetrapod, 200 reverse joint, 300-302 tank): BORNEMISSZA 53300000, VE-42B 53340000, FORTALEZA 53360000. The param
  is found in the repository by name (`param_index`: res+0x18 is an MSVC std::wstring, len +0x10, cap +0x18, heap
  pointer when cap >= 8). Vanilla, equipping tank legs sets the booster slot's index/handle/id to FFFFFFFF ("Booster
  retiré. Les jambes de type tank ont des boosters internes."); leaving them installs a booster from the inventory.
- **Vente** refuses equipped parts ("Vente des pièces équipées impossible.", listed greyed with an EQ badge), and the
  garage auto-sell uses `stash()` (equipped handles excluded): equipped parts are never sold.
- **Stale index.** A weapon slot can hold an inventory INDEX with an empty handle/id (seen in the CE-made gear_test
  save): the Assemblage then shows whatever item later takes that index (a granted FINDER EYE showed in a back slot).
  The penalty now clears index + handle + id of all four weapon slots.
- **Dialog.** Starters lost and bought back only count in "Pièces de départ facturées" (a 7-slot list overflowed the
  box onto the OK button).

**Verified in game** (fresh3 .gear_test, failures by killing the game mid-mission, penalty on load):
1. weapon + 2 non-starters + 5 starters, spare starter FCS, no spare head (sold in Vente): GU-A2/LAMMERGEIER/TALBOT ->
   lost rows 207/84/105, FINDER EYE bought + granted + equipped, SCT spare equipped, 5 starters kept, cost 663 000,
   BU-TT/A auto-sold, 650 000 -> 2 000.
2. tank legs as newest spare: BORNEMISSZA equipped, booster slot FFFFFFFF, CRAWLER + BST removed (not lost forever),
   TURNER auto-sold, 540 000 -> 4 750; the shop then offers CRAWLER and BST-G1/P10.
3. tank legs equipped, no spares: BORNEMISSZA lost (row 18), CRAWLER + BST bought and equipped, 774 000.
4. starter head with two spare heads (VE-44B then the cheaper TIAN-QIANG): TIAN-QIANG equipped for free, FINDER EYE
   removed, VE-44B sold (the replacement never is), 700 000 -> 44 500.
5. unaffordable failure (-729 500, nothing to sell): game over at load, back to the title.
Auto-sold parts enter the buy-back list through task 2's `buyback::record` in `sell`; lost parts never do (removed with
RemoveItemAt, not the Vente path). Not done: real in-mission deaths not replayed
(same plan / apply code as the kill path). Debug commands: `give <hex item id>`, `lost`.

## Pool without repeats (2026-10-27, task 4)
**Superseded (2026-10-28) by "Duplicate parts in the shop": the rolled sets, refills, credits fallback and pool_limit
are gone.** Kept for the history.
Two pools (weapons, other parts) over the buyable lineup rows; each row is its own part (left/right weapon versions
included). Candidates = in the pool, not in `ROWS_ROLLED` (7100..7417), not owned, not in `ROWS_LOST`. Rarity unchanged
(weighted tier among the tiers that have candidates, uniform within).
- Shop (`pick_shop`, at the reroll): shop_weapons then shop_parts slots; every pick sets its rolled bit. A pool with
  no candidate is refilled once (its rolled bits cleared except the rows picked in this roll and the sortie's
  rewards) and completes from the refill; a slot still without a candidate stays empty.
- Mission reward (`pick_reward`, at sortie): both pools, also excluding the current shop rows. Nothing left: refill
  both (keeping the current shop rows). Still nothing (all owned / lost / on offer): a part rolled among every
  sellable part is paid as its sell value (`penalty::sell_value`). Rewards set their rolled bit only when given, at
  5074 (mission cleared), with the reroll: a failed sortie leaves the pool untouched.
- Credits of converted rewards: `penalty::CREDIT_REQUEST`, added to COAM on the game thread at the mission clear
  (the frame hook handles it before the garage-only early return), i.e. before the commit charges ammo/repair (task 1
  lesson). A dialog ("PLUS AUCUNE PIÈCE DISPONIBLE ...") is requested when the result screen closes (flag 3139 1->0).
- Config `[shop] pool_limit` (test knob): each pool keeps only its first N rows.
Verified in the garage with pool_limit 3 (rows 124/125/126 weapons, 4/5/6 parts) through debug commands (`reroll`,
`rolled`, `reward` = a sortie's reward roll + its mission clear without the grant, `save`): parts pool 2 per roll →
2nd roll takes the last one, refills ("2 back in", keeping it), completes with a non-repeat; weapons refill on the 4th
roll; reward picks an unrolled non-shop row and marks it; with the rest owned (`give`) the reward refills, finds
nothing, converts DIZZY to +65 000 COAM (527 790 -> 592 790) and shows the dialog. Persistence: rolled [4, 5, 126] +
shop survived save, kill, relaunch, Continue.
Real missions: a story clear (Grid 086 area, user-played) gave reward row 118 through "Pièces obtenues", marked it at
5074, rerolled a shop skipping every rolled row; the commit autosave kept shop + rolled set (read back after a
restart). Credits path in a real sortie (pool_limit 3, everything else owned): the reward converted at sortie
(HD-011 MELANDER); 5074 raised BY HAND mid-mission (tools/flag_test.py) made the DLL pay +18 750 on the game thread
mid-mission (22 094 -> 40 844, seen by the COAM detour) and reroll — but the game did NOT end the mission: 5074 is an
effect of the clear, not a trigger. Still unobserved: the converted-reward dialog after a real result screen (3139↓).
Arena fights: no 5074, the queued reward is given silently at the end (separate task). Note: one debug `save` request right after a Continue was not written (cause unknown,
file unchanged); the next one was.

## Native reward choice: menu RE (2026-10-28, task "pick 1 of 3")
Goal: a native dialog (grid of parts, a TAKE button each). Menus are Scaleform GFX v11 files `menu/<name>.gfx` (me3 can
override them; the logo-skip mod does). `tools/ac6_unpack.py` extracts single files from Data0-3 (BHD5 RSA keys read
from UXM's exe, AES ranges); FFDec 26.4 in `Modding\PC\Tools\ffdec` decompiles them (`-export script`, `-dumpSWF`).
- `02_gotreward.gfx` = "Pièces obtenues" and every other reward popup: AS3 is display-only (frame stops, named slots
  Title/Button/Thumbnail/Message/HitArea); the C++ class (code ~exe+1012000-1016000, Arxan-split) drives it.
  Menu names table: exe+4E28CF0 (16-byte entries, GotReward = id 0x46, name ptr at exe+4E29158).
- Menus are created from `std::function<RefPtr<MenuJob>(ComponentStack&)>` objects (MSVC: 0x38 storage + callable
  ptr at +0x38; callable = {vtable, captures...}): `exe+DA3410(&job, &func)` makes the job, `exe+DC2A70(&job)` hands
  it to the menu manager (`[exe+4E15530]+0x18`, exe+D834E0) and releases it. `dll/src/rewardmenu.rs` does this from
  the garage game thread (debug `gotreward <vtable rva> [item hex] [capture hex]`).
- GotReward factory vtables (exe+392B500..392B920, call slot -> class function), fired from the garage:
  392B7D0/392B590/392B6B0 (no capture) -> "Début de la phase de promotion"; 392B770 (int capture = emblem id) ->
  "Emblème acquis"; 392B800 -> "Fin de la phase de promotion"; 392B860 -> "Plaque obtenue"; 392B890 (0x14 bytes) ->
  "Progression dans Loghunt"; 392B8F0 (captures a RefPtr to a data object; zero capture CRASHED the game) -> likely
  the parts list ("Pièces obtenues", class fn exe+1015640); 392B8C0 (moved ptr + int) and 392B920 (8 bytes) untested.
- RE tracer: `dll/src/trace.rs` = hardware breakpoints on every thread + VEH log (regs, [rcx..], return chain);
  debug `watch <slot> <addr|exe+rva> <x|w|rw> <len> [max]` / `unwatch`; `%USERPROFILE%\ac6_watch.txt` = commands
  run at boot; re-armed at sortie and at the mission clear. Screenshots: grant `tabtip.exe` (an invisible
  full-screen TabTip window otherwise masks the whole screen).
- **"Pièces obtenues" with our own list: WORKS (2026-10-28).** Real flow: results screen lambda exe+E9A220 builds a
  `vector<u32>` {allocator (exe+2923D10()), begin, end, cap} with `exe+E9D420(results, &list)` and calls
  `exe+10158D0(&job, ctx, &list)` (the parts mode of the GotReward class; title code exe+10142B9 runs from
  exe+1015973). Entries = the tagged item ids the DLL grants with (`Part::grant_id`: weapon bare id, frame
  0x10000000|id, booster 0x6.., FCS 0x7.., generator 0x5..); a bad id shows "(id rounded to 100) +N". The DLL's own
  std::function (vtable in rewardmenu.rs: _Copy, _Move, _Do_call(this, out, ctx), _Target_type, _Delete_this, _Get)
  calls exe+10158D0 with our list; pushed like the others it opens a native one-part-per-page dialog (OK = next page),
  display only (nothing granted). Debug: `parts <id hex> ...`. Menus pushed while another modal is up wait for it.
- **How the class fills a part tile** (seen live with a breakpoint in the filler while our dialog opened): in the
  parts update (around exe+1012CA8..1012D1C, game/menu thread = our garage hook thread):
  `exe+C92E20(&comp, parent)` -> component handle, `exe+1001530(&view, comp)` builds a part view (binds the
  children with `exe+C928E0(component, &out, "CategoryName" / "MakerLogo" / "ItemName" ...)` = get child by
  instance name), `exe+1000B60(&view, item)` fills it, `exe+E83BE0(&view)` destroys the view (~0x8A0 bytes,
  stack). Grid plan: a modified 02_gotreward.gfx with K copies of the parts content symbol (Symbol 218: MakerLogo,
  ItemIcon/Thumbnail, CategoryName, ItemName) as TileNN, filled by the DLL with these calls; the tiles themselves
  selectable (controller cursor + mouse HitArea), max 3 columns x 4 rows (user, 2026-10-28). Open: the item argument
  of exe+1000B60, the framework's button/focus component (Button + HitArea + Cursor) and its press callback.
- **Own movie file: WORKS (2026-10-28).** `mod/menu/02_rwdchoice.gfx` (me3 `[[packages]] path = 'mod'`) is
  preloaded under the menu table entry of 02_OnlineArena_GotReward (exe+4E29168 name ptr -> our UTF-16
  "02_RwdChoice", set at boot before the title preload exe+1604BCC). Our factory writes "02_RwdChoice" over the
  class's name string exe+39BF5C8 ("02_GotReward", same length) and restores it ~60 garage frames later (the class
  reads the name after the factory returns). Test file = vanilla layout with `Dialog` scaled 0.6 in MainTimeline
  frame 1 (FFDec `-importScript`): the dialog came up scaled, vanilla "Pièces obtenues" unchanged.
- **Native grid filled: WORKS (2026-10-28).** Each "page" of the parts dialog is a fresh window made by the factory
  exe+1012BB0 from a page object whose content `std::function` (page+0x48, _Impl ptr at page+0x80, callable vtable
  exe+39BF948, record at +8 copied with exe+E837C0) fills the part view. The page function (class+0x4F0 = window
  root, +0x558 = button) does GetChild(root, "_Content_") (call exe+1010A44, rsi = page), `exe+C94640(&h)`, then
  `_Do_call(callable, &h)`. For the grid: our factory also builds one hidden single-part job per part with
  exe+10158D0 (never pushed) and finds each job's content callable (search for the vtable); when our dialog binds its
  content (hooks on exe+1012CD4 / 1012CE4 / 1010A44), for each part: GetChild(root, "_Content_/TileNN"),
  C94640, `_Do_call(callable_i, &tile)`, `exe+C6A370(&tile)` (handle dtor). "_Content_/SelIdx" gets
  `exe+C8CBA0(handle+8, L"sel/count")` (SetText); the movie's AS3 lays out count tiles (3 columns) and grows the
  window. The real dialog gets only the first part, so its OK closes it. Tools: tools/build_rwdchoice.ps1 (+ .py),
  mod/src/rwdchoice_MainTimeline.as, tools/relaunch.ps1. The movie's AS3 receives NO input (keys, mouse).
- **In the results flow: WORKS (2026-10-28).** Opening the grid on our own at the clear (5074) popped it mid-mission
  and the commit closed it. Now: the offers (parts_per_mission x reward_choices parts) are rolled and queued at sortie
  like plain rewards; the results lambda's call `exe+E9A26E -> exe+10158D0(&job, ctx, &list)` (list = the pending
  parts) is hooked and opens our grid instead, one page per offer (+ one per other pending part, e.g. a story reward);
  the highlighted tile of each page is its pick; the commit `exe+B042FC -> exe+4756E0(mgr, 1, obj)` is hooked and
  first erases the other offered handles from the pending vector (mgr+0x80/+0x88, u32 handles), so only the picks
  are inserted. Pages: one class, N pages (OK = next page); a page is recognised by its page object (rsi at the
  content lookup), filled at the class's next part fill. The movie name stays ours until the last page is up.
  On the "AC d'essai" results screen the parts step waits for Fermer (pending list intact until then).
- **Pad: NOT via XInput.** XInput sees the pad from outside the game (slot 0, Bluetooth Xbox), but inside the process
  the DLL's XInputGetState calls report nothing and the game never calls its XINPUT9_1_0 import: the game reads the
  pad through its own input system (probably Steam Input). Solved with the game's menu actions (below).

## Menu input (2026-10-28): the game's menu actions drive the grid
Found by memory diff (tools/input_scan.py: snapshots while an arrow key is held / released, 7 GB, ~20 candidates in 6
rounds; tools/input_watch.py / input_dump.py to watch them; tools/ptr_scan.py for pointer chains):
- A float array (one per virtual input id, -1 = idle) holds the menu auto-repeat timers; down = id 0x15C. Written
  every frame by exe+29F23F0(repeater rcx, input rdx, ?, ?, f32 dt): for each repeat entry {f32 delay 0.2, f32 repeat
  0.067, i32 id} (vector at [r9-derived obj+0x10..0x18]) whose bit is set in the bitset [input+0x7C8], the timer runs and
  the bit is CLEARED between repeats -> after the call, [input+0x7C8] = this frame's menu actions (press + repeat
  pulses). Called per device mapper from exe+2A39420 (virtual [obj+0x20], input object = r13) under the input manager
  loop exe+29EC4xx ([mgr+0x770..0x778] = mappers). Keyboard and pad both feed it (same ids).
- Ids: confirm 0x155, back 0x154, right 0x156, left 0x159, down 0x15C, up 0x15F (each direction also raises sibling
  ids, e.g. right 0x137/0x138/0x157/0x158; the title/garage menus show more: 0xD0.., 0x100.., 0x12E..).
- The DLL hooks exe+29F23F0's entry (15-byte prologue moved to a trampoline), ORs [input+0x7C8] words 0..15 after
  each call, and the grid's tick takes them: directions move the highlight, confirm closes the page (the class's OK
  does the rest). No XInput, no keyboard polling: the pad, the keyboard, the game's own repeat timing.
## Test routine: instant mission clear (2026-10-28)
`python tools/kill_enemies.py --max-ap 12300` (game running, in the mission, 0.4 s) zeroes the tester AC's AP and the
game kills it at once (no hit needed). Characters come from WorldChrMan `[exe+4E0F598]`: its lists `[WCM+0xAB58]`
(player = entry 0, opposing ACs: tester AC = entry 1), `[WCM+0xAB78]` / `[WCM+0xA760]` (scripted objects) hold ChrIns
pointers; ChrIns `+0x320` -> modules, `[0]` = AP block (AP i32 +278, max +27C, same class as the player's). Without
options the script lists them; `--write [--only <block>]` zeroes the listed ones; `--scan` falls back to a full memory
scan (~45 s; `tools/chr_paths.py` re-finds the lists if an update moves them).
- "Destruction de l'AC d'essai" (chapter 1, the tester AC): blocks AP 12300 (the AC: zero only this one), 750 and
  99999 (scripted objects, leave them). Zeroing the AC -> mission cleared, results, rewards as usual.
- Save right before it, in the repo: `saves/ac-mission-available.sl2` (garage, no sortie pending).
  `tools/restore_save.ps1` puts it in place of the dev save (game closed; the current save is kept first).
- Scripted: `tools/restore_save.ps1 -Save <save>` -> `tools/relaunch.ps1 -Profile <profile>` -> `python
  tools/title_to_garage.py` -> `python tools/garage_to_mission_success.py` (garage -> mission -> kill -> reward grid; they
  keep the game in front; restore the save each time: the cleared mission leaves the list). Menus need a hover before
  the click (a click on the frame the cursor arrives only highlights the entry); every step waits for its screen (pixel
  probe, timeout). The briefing (~50 s, first view per game session) has no keyboard skip found: its "Passer" prompt
  shows the pad's Menu button; Esc / Enter / Space / Backspace / Tab / F / Q tapped, doubled or held do nothing.
- Full cycle by hand: `tools/restore_save.ps1` -> `tools/relaunch.ps1` -> title OK + CONTINUER -> DÉPLOIEMENT -> MISSIONS ->
  "Destruction de l'AC d'essai" -> briefing -> Déploiement -> OUI -> wait for control -> `kill_enemies.py --max-ap 12300`.
- Keep `tools/save_backup.ps1` running while testing; never kill the game mid-sortie (escrow -> failure penalty on the
  next load), restore the pre-sortie backup instead.

## Duplicate parts (2026-10-28, research)
Question: can a reward give a part the player already owns (a second copy) instead of the credits fallback?
Tested on an isolated save (copy of saves/ac-mission-available.sl2, profile ac6-roguelike-dup.me3), DLL off then on.
New debug commands: `give2 <hex id>` (direct give even if owned, then logs the duplicates), `force_reward <hex id>`
(the next sortie's first reward / first offer is that part, owned or not), `dups` (inventory entries of every part
owned more than once: index, handle, count, ordinal, EQ).

**Vanilla.** The shop never sells a second copy: the buy dialog has no quantity, and the ownership filter hides a part
as soon as one copy is owned (bought HG-003 COQUILLETT with the DLL off: gone from Achat at once). Vente's tutorial:
parts sell at their buy price and can be bought back in Achat (vanilla buy-back = the same "not owned" filter).
Left/right versions are separate rows, so dual-wielding the same weapon = owning both rows, never two copies of one.
The arm+back "weapon bay" (OS Tuning unlock, not on this save: back lists showed back units only) would be the only
way to equip the same row twice; untested.

**The give paths accept a second copy (static + live).** Give wrapper exe+46B1C0: `exe+BA9420(id)` = "stacks?" is
true only for goods (0x4…) and weapons whose EquipParamWeapon.weaponCategory (+0xE6) is 13/14 (Arrow/Bolt, engine
leftovers: no AC6 part). Stackables go through the find-by-id `exe+474060` + add-count `exe+473990` (status 2 =
refused/overflow at obj+0xBCC). Everything else (weapons, frames, internals) goes straight to AddItem `exe+474D80`,
which has no duplicate check: a NEW table entry every time (count 1, next ordinal), or the pending queue when the
defer byte is set. Purchase (`exe+473EE0`) calls the same core.
- Direct give (garage): COQUILLETT, VE-44A head, TALBOT FCS each got a second entry, rax = new index, status 0.
- Deferred grant at sortie + commit: owned TALBOT forced as offer 1 of the reward grid → queued like any part
  (rax=-1, pending handle 0x742C4620), shown in "CHOISISSEZ UNE PIÈCE" with no owned marker, picked → the commit
  inserted entry #87 (ordinal 28). The other 2 offers were pruned as usual.

**Handles: weapons/frames unique, internals NOT.** Weapons get `0x08000000|serial`, frames `0x18000000|serial`, one per
copy (renumbered at each load). Boosters / generators / FCS use **handle == item id** (like goods), so two copies of an
internal share one handle (both FC-008 TALBOT entries: 0x742C4620). Counts stay 1 per entry; no per-item limit, the
table holds 0x800 entries (mgr+0x08; parts start at index 64).

**Game UI handles copies by inventory INDEX, fine.** Assemblage lists every copy as its own tile (EQ on the installed
one) and the slot INDEX array follows the chosen copy (FCS slot 25 idx 0x59 = 2nd copy, handle array identical).
Vente lists every copy, equipped one greyed with EQ, and removes exactly the chosen copy, even for internals with a
shared handle (sold TALBOT #89 while #83 was equipped: #89 freed). Two copies of a frame part / internal can never be
equipped at once (one slot each). Duplicates survive save + reload (same indexes; handles renumbered).

**Mod systems keyed on "owned".**
- Shop ownership filter (vanilla) + `owned_ids` / pool candidates / `choice::roll` / starters always buyable: a
  duplicate only ever ADDS ownership, so these keep excluding the part; after selling one copy the part stays hidden
  while another copy is owned. Nothing ever offers a part already owned unless forced.
- Buy-back: a Vente sale of a copy records its row (sold COQUILLETT → row 166, TALBOT → row 105) but Achat does not
  show it while another copy is owned; it appears only if the last copy goes before the reroll. Harmless.
- Failure plan / auto-sell (`penalty.rs`) identify parts by HANDLE (`equipped_handles`, `stash`, `newest_spare`,
  `find_handle`, `remove_handle`):
  - weapons / frames: correct. Verified: VE-44A equipped + spare copy → "lost VE-44A forever", "replaced by VE-44A"
    (the spare copy, newest ordinal), so the row is in ROWS_LOST while the player still has that part.
  - internals: every copy looks equipped. The spare copy is never a spare (verified: TALBOT lost, replaced by the
    older SCT-G1/P01 starter instead of the newer TALBOT copy; with no other FCS the plan would BUY the starter),
    never in the stash (auto-sell and the game-over check ignore its value), and `remove_handle` removes the first
    entry with that handle (verified: the unequipped TALBOT #83 was removed, the lost one #87 stayed in the
    inventory). Net counts are right by luck; the player keeps a TALBOT whose row is "lost forever".
- Mission-reward prune at the commit erases unpicked offers BY HANDLE from the pending vector: an unpicked internal
  offer would also erase another pending copy of the same internal (e.g. a story reward of the same FCS). Rare.
- Inventory cap (parallel task): must count entries (each copy is one), not distinct ids.

**Recommendation for "give a duplicate instead of credits".** Feasible with the existing grant path (no game-side
blocker), but before switching the fallback: (1) make penalty.rs identify parts by inventory INDEX (or handle+index)
instead of handle, so internal copies are spares / stash / removed correctly; (2) decide whether a lost row with a
copy still owned should go to ROWS_LOST (simplest: only mark it lost when no copy is left); (3) prune pending offers
by position, not handle value; (4) count copies in the cap. Safe today without changes: duplicate weapons and frame
parts (unique handles); the risky case is internals.

**Decision (user, 2026-10-28, for the next pool task):** the "don't roll what's already out" pool goes away, so weapons
and frame parts (head/core/arms/legs) may roll again and be duplicated; boosters, generators and FCS keep being
excluded from rolls once owned (their copies would share a handle). Handle evidence per slot: two copies tested for a
head (VE-44A, serial handles 0x1800001E / 0x18000021) and an FCS (TALBOT, both 0x742C4620); the other slots read from
the table: core/arms/legs starters 0x18000018/19/1A (serials), booster 0x63949A00 and generator 0x53DFD240 (= id).
Consequences for that task:
- Rewards (direct or deferred give, reward grid, commit prune) handle duplicate weapons/frames as tested.
- **The SHOP cannot show an owned part as is:** the game's ownership filter (`filter->Accept` in the per-frame loop
  exe+E928F0, see "Ownership filter") hides every owned row whatever `shop_pred` returns, so a rolled duplicate leaves
  an empty slot (same for buy-back and the starters). Showing duplicates in Achat needs that filter found and
  bypassed for the chosen rows (its predicate is not identified yet: exe+E88B10 jumps through [filterObj+8]).
- penalty.rs stays correct for weapons/frames; with internals never duplicated, the shared-handle problems above do
  not arise (the starters are only re-granted through `ensure_owned`, which skips owned ones).
- Still to decide: a lost weapon/frame row goes to ROWS_LOST even when the player keeps another copy.

## Duplicate parts in the shop (2026-10-28, implemented)
Decision applied: the pool without repeats is gone. Shop offers and rewards draw by rarity among every buyable row;
weapons and frame parts (head/core/arms/legs) roll even when owned; boosters / generators / FCS ("unique", copies would
share a handle) never while owned nor once in `ROWS_LOST`. Within one roll the parts stay different; the shop leaves
out the parts being awarded at that clear (see baselines), rewards leave out the unique parts of the current shop.
No credits fallback, no `pool_limit`.
- **Lost forever** (user, 2026-10-28): only boosters / generators / FCS (non-starter) go to `ROWS_LOST` on a failure. A
  lost weapon / frame part is just removed and may roll again. Old saves' `ROWS_LOST` bits of weapons / frames are
  ignored (the candidate filter checks `ROWS_LOST` for unique parts only).
- **The shop's ownership check = one function**, exe+D2F900(`const u32* id`) -> 1 if the id is not in the inventory
  (exe+474060 < 0), else 0: "how many can you still get". Users (all shop menus): the buy list's filter exe+E89820
  (via wrapper exe+D2F860; the list filter std::function built at exe+641D108, vtable exe+395BF20, fn ptr at +8, run
  per entry by exe+E928F0, entry item id at +0x54), the purchase confirmation exe+10A9690 (call at exe+10A96FE), the buy
  step exe+D669C5 (via D2F860; 0 -> message 231020 "Vous avez acheté toutes les pièces disponibles.", text set up by
  exe+D685A0), the purchasable quantity exe+D2F480 (call exe+D2F4DF: min(stock, affordable, obtainable) for
  non-stackables) and the adds exe+D2EC20 / D2ECB0 (calls D2EC51 / D2ECE1: n = min(D2F900, requested) then add).
  Bypassing only the filter + confirmation made the game charge the price and add nothing (quantity 0): the hook must
  sit on D2F900 itself.
- **`dupshop.rs`**: D2F900 hooked at its entry (10-byte prologue to a trampoline). For a shop slot's weapon / frame
  part it answers `copies owned <= baseline`, the baseline being the copies owned when the slot was rolled (3 bits
  per slot, flags 7100..7123, written at the reroll / debug `shop`). Listed and bought like any part; once bought the
  count exceeds the baseline and the slot vanishes like a vanilla purchase (until the next reroll). Every other id
  gets the game's answer (buy-back and starters keep the vanilla "not owned" rule).
- **Save format**: flags 7124..7131 = 0xA5 once `dupshop::migrate` cleared the old rolled ranges (7100..7417,
  7501..7799, 7838..7856), checked every DLL tick (a New Game resets it, the clear runs again on empty flags).
- **Verified** (isolated profile ac6-roguelike-pool.me3 -> AC6_roguelike_pool.sl2, copies of the dup save and of
  saves/ac-mission-available.sl2): debug `shop 993388 12FB8CC0 742C4620` (owned XUAN-GE, owned + equipped VE-44A,
  owned TALBOT) -> XUAN-GE and VE-44A listed, TALBOT tab empty; XUAN-GE bought (366 000 -> 296 000, second entry, own
  handle 0x08000021), slot gone at once; save + relaunch: XUAN-GE still hidden, VE-44A listed and bought (second
  entry, handle 0x1800001C). Failure (escrow set in the garage + reload): XUAN-GE (spare copy owned), VE-44A (replaced
  by its spare copy), VE-46A and TALBOT lost; only TALBOT's row 105 entered `ROWS_LOST`. Real sortie ("Destruction
  de l'AC d'essai", kill_enemies.py): migration ran, offers rolled, reroll wrote baselines [0, 0, 0]; the forced owned
  VE-46A picked in the grid became a second copy.
- **Core expansions (user asked to add them):** done, see "OS Tuning rewards" below.
- Not changed: buy-back of a sold copy stays hidden while another copy is owned (vanilla rule, `dupshop` only covers
  shop slots); the reward-choice prune drops unpicked offers by handle (safe: unique parts are never offered owned).

## OS Tuning rewards (2026-10-28, implemented; replaces "core expansions as rewards")
User decisions (2026-10-28): every OS Tuning is a mission reward, offered again while below its max level (a pick of
one already installed adds a level); core expansions epic, the other tunings rare; the grid tile shows the level it
brings; the vanilla OS Tuning shop stays locked. Losing equipped OS on a failure + skipped OS never offered again: done
next, see "Core expansions: lost on failure, offered once" below.
- **Levels** (TGA table "OS Tuning", verified live): `[PlayerGameData+0x1368]` -> one i32 per OsReinforce row, in
  row id order (offset = 4 x rank): 10 kinetic +0x00, 20 energy +0x04, 30 explosive +0x08, 40 melee +0x0C, 50 direct
  hit +0x10, 1010 ACS +0x14 (1011..1013 = +0x18..+0x20: hidden sub-rows without text, not rewards), 2020 repair kits
  +0x24, 3010 access speed +0x28, 4010 weight control +0x2C, 5010 ASSAULT ARMOR +0x30, 5020 PULSE ARMOR +0x34, 5030
  PULSE PROTECTION +0x38, 5040 TERMINAL ARMOR +0x3C, 5050 boost kick +0x40, 5060 manual aiming +0x44, 5070 weapon bay
  +0x48, 5080 quick turn +0x4C. 0 = not installed. Saved in the .sl2.
- **Max level** = OsReinforce levels with an OST chip cost > 0 (5 entries {increase, cost, text id} from +0x14, 0x18
  apart): 10/20/30/1010 = 5, 2020 = 4, 40/50/5010/5030 = 3, 3010/5020 = 2, the rest 1 (same as TGA's "Max all").
  OsReinforce is repo index 202; it is not read when Assemblage's EXTENSION list opens.
- **Names**: MsgRepository category 200, ids 71510 (kinetic) 71511 (energy) 71512 (explosive) 71513 (melee) 71514
  (direct hit) 71520 (ACS) 71531 (repair) 71540 (access) 71545 (weight) 71550 (boost kick) 71551 (weapon bay) 71552
  (manual aiming) 71553 (quick turn) 71560..71563 (assault armor, pulse armor, pulse protection, terminal armor).
  Category 73 = garage menu titles (2009000 "AMÉLIORATION OS"), 73/74 id 2101000 = the unlock notification title/text
  ("Accès autorisé : AMÉLIORATION OS/ARÈNE", "...disponibles : • AMÉLIORATION OS • ARÈNE").
- **Assemblage EXTENSION list = expansion item owned AND level > 0** (item alone: (NÉANT); level alone: (NÉANT); level
  set back to 0 hides it even equipped). The vanilla OS Tuning menu shows a rewarded level as installed (cyan segment).
- **Menu unlock**: flag 3131 (set by mission 9's end script with 3130) = UnlockParam_MenuCommand row 10080 {type 2,
  flag 3131}; with 3131 set the garage gets "AMÉLIORATION OS" after a reload (no dialog), first open shows a tutorial.
  Lock: the DLL points row 10080 at flag 8020 (the game's own never-set flag of row 99999999; block 8 empty), at boot
  and with the economy re-apply. Verified: 3131 = 1, menu absent after a mission and after quit + Continue.
  Notification: the msg hook serves 2101000 without the menu (title: "<menu>/" cut; text: lines naming it dropped),
  built from the game's own texts per source string (they are reloaded in the game language after the title):
  "Access Granted: ARENA" / "...• ARENA" checked by debug `osnote`. The real notification (the mission whose end
  sets 3130/3131: the user recalls the Balteus one, Attack the Watchpoint, not checked) is NOT seen yet.
- **Texts naming OS Tuning / AOS chips** (full dump of the live MsgRepository; `fmg_search.py` cuts texts at 300 chars):
  ALLMIND's garage message granting OS Tuning + Arena (cat 1, 913001010..070: service record updated / **OS Tuning granted** /
  "En outre..." / Arena invitation x4), the Arena tutorial (74/2004000: arena wins give AOS chips), the OS menu
  tutorial (74/2009000, never shown while locked), the total load help (205/75230, weight control tuning). No mail.
- **Voice is per line**: each TalkParam row (repo slot 32, 6013 rows) = one line {msgId +4, voiceId +8}; the message
  is rows 913001010..016 (msgIds 913001010..070 by 10, voiceId = msgId). The player walks the rows BY POSITION for a
  line count it does not take from the rows: removing headers (even at boot) or renaming ids made Walter's tester-AC
  message (910450010..013) play the next rows in the array (another message's lines, and "Réception d'un message"
  instead of "Trois nouveaux messages"). Blanking a row (msgId and voiceId -1) skips it cleanly: Walter went from line
  1 straight to line 4, same notification count, no gap. `os::cut_allmind_talk` (boot + param re-apply) blanks rows
  011/012 -> ALLMIND plays lines 1, 4-7. Verified on Walter's message (same code path), the ALLMIND one itself not
  seen (needs a real return from the unlocking mission).
- **Garage return after the unlocking mission** = the garage talk script (EzState, found by scanning memory for 913001010): it
  queues the ALLMIND message (speaker 91000010) and itself sets 3130/3131 + shows notification 2101000 (lists "3130 1
  3131 1 2101000 7"; mission 2's block likewise sets 3127/6200 + 2100000). Setting 3130/3131/3451 during the tester AC
  mission does not reach that block: no notification, no ALLMIND message; the garage then lists ARÈNE (3130, no
  reload needed) and no AMÉLIORATION OS.
- **Grant** (`os.rs`, `choice.rs`): OS Tunings join the reward pool (`[rewards] os_rewards = 1`; Part kind KIND_OS, idx
  1000+k, never a shop candidate) while level < max. "Pièces obtenues" only opens on a non-empty pending list, so every
  OS offer queues an item at sortie: its expansion item when not owned (`keep`: committed if picked), else a carrier
  (PULSE ARMOR item), always taken out of the list in the commit hook. The pick raises the level by 1 there (direct
  write). With reward_choices = 1 a reward that is an OS Tuning still goes through the grid (one-tile page "PIÈCE
  OBTENUE") so its carrier is removed.
- **Tile**: the part view is filled from the queued item, then the DLL writes `_Content_/TileNN/ItemName` (tuning name)
  and `CategoryName` ("AMÉLIORATION OS  LV n/max", n = level after the pick); `SelIdx` carries a 3rd field, a bit mask
  of tiles whose ItemIcon / MakerLogo the movie hides (non-expansion tunings: no image).
- **Verified** (profile ac6-roguelike-exp.me3, fresh copy of saves/ac-mission-available.sl2, 3 rewards per mission,
  debug `force_os 5050 5020 5010` with PULSE ARMOR owned at level 1 and 3131 set): grid pages showed "Coup de pied
  propulsé  LV 1/1" (no image), "Bouclier à impulsions  LV 2/2" (chip image), "Bouclier d'assaut  LV 1/3" (chip image);
  picks -> levels 0->1, 1->2, 0->1, pending 9 handles -> only the assault armor item kept; Assemblage lists one assault
  armor + one pulse armor; boost kick and pulse armor no longer rollable; levels kept after quit + Continue.
- Penalty unchanged here (expansions keep no slot in FRAME_SLOTS / WEAPON_SLOTS, price 0): kept after a failed sortie. Superseded by [lost_on_failure] (section below).
- Debug: `os` (level / max / rollable / item owned per tuning), `osnote`, `force_os <rows>`, `force_reward <hex ids>`
  (now several: first offer of each reward).

## Core expansions: lost on failure, offered once (2026-10-29, implemented)
User decisions (2026-10-28): two per-item config tables instead of one switch, keys like [part_rarity] (equipId,
part name, OsReinforce row for the expansions), value 1/0, unlisted = the old rules. Shipped with the four core
expansions (5010..5040) = 1 in both.
- **`[lost_on_failure]`**: 1 = the equipped item is lost on a failed sortie, 0 = it survives (stays equipped, no
  replacement, no charge). Unlisted: weapons / frame parts / internals lost, core expansion kept. Non-expansion OS
  Tunings are never lost (ignored with a log line; superseded: one level lost, see "OS Tunings: one level lost on
  failure"). A lost expansion: slot emptied, item removed, level -> 0; the loss dialog gets "Extension perdue : <name>".
- **`[unique_occurrences]`** (the user merged "lost forever" and "skip is final" into this): 1 = offered at most once
  per run, picked or not. Rewards are marked at 5074 (every offer of the sortie: the results screen shows them all,
  before the commit autosave), shop parts at the reroll. A listed item lost on a failure is marked too. Among the OS
  Tunings only core expansions can be listed. An expansion picked once stays at level 1.
  Superseded by [max_occurrences] (section below).
- **Equipped expansion = assembly slot 11**: ID `[PlayerGameData+0x520]` = plain item id (75000000 Assault Armor),
  index `+0x278`, handle `+0x450` (same deltas as the other slots; verified live: index 0x56 -> inventory entry with
  that handle and id). Slot 22 (`+0x54C`) is not it.
- **Storage**: lineup parts reuse `ROWS_LOST` (4550 + row: "never offered again", read by `candidates` for every part
  now, not only boosters / generators / FCS); OS Tunings: `OS_GONE` = 4418 + rank in OS_TUNINGS (17 flags,
  4418..4434). Block 4 on the exp save several missions in: only 4901..4964 (reward RNG) set. New Game clears block 4.
- **Verified** (profile ac6-roguelike-osloss.me3, fresh copy of saves/ac-mission-available.sl2, parts_per_mission 2):
  `force_os 5010 5020` -> both shown in the grid, "never again" logged at 5074, both picked (levels 1/3, 1/2) and no
  longer rollable (debug `os`), still not rollable after a relaunch. Assault Armor equipped (slot 11), COAM set to
  2 000 000, escrow 7828 + save, relaunch + Continue: penalty "core expansion Bouclier d'assaut (5010) lost", level
  1 -> 0, no 75xxxxxx item left, Assemblage EXTENSION (NÉANT), dialog line shown. Same save with `"HI-32: BU-TT/A" = 0`
  and `"HC-2000 FINDER EYE" = 0`: L-arm weapon and head kept, the rest lost as before, expansion still lost.
- Test note: the first attempt had 58 906 COAM for a 534 000 failure -> game over at load (unaffordable failure rule),
  so give the test save money before the failure.

## Parts out of the pools: tier 0 / weight 0 (2026-10-29, implemented)
(Tiers replaced by named pools on 2026-10-29, "Pools in the settings file": `= 0` and weight 0 still put parts out.)
User request (2026-10-28): unlisted parts or parts whose pool is 0 are kept out of the pools, so pools can be made small
on purpose. User decisions: no credits fallback (an empty pool just gives nothing: shop slots stay empty, an offer shows
the parts left, an offer with none is dropped); OS Tunings follow the same rule.
- `[rarity] unlisted_tier = 0` and `[part_rarity] <part> = 0` put parts out; a tier whose weight is 0 is out too.
  `Part::in_pool` (tier 1..3 with a weight > 0) is checked by `candidates` (shop, plain rewards, reward choice); excluded
  parts stay in `Run::pool` for lookups (a shop slot rolled under another config shows until the next reroll, debug
  `shop` / `force_reward` still find them). `pick_weighted` no longer falls back to a uniform pick when every weight is 0.
- Boot log: `rarity: N common, N rare, N epic, N at tier 0; weights a/b/c` and `pools after exclusion: shop weapons N,
  shop parts N, rewards N (N OS Tunings); N part(s) left out`. Debug `choice` with an empty pool logs "no grid".
- **Verified** (profile ac6-roguelike-excl.me3, copies of saves/ac-mission-available.sl2): config with unlisted_tier 0,
  epic weight 0, ZIMMERMAN 1, BAD COOK 2, TIAN-LAO 2, VERRILL 3 (out), VE-60SNA 0, Boost Kick 2, Assault Armor 3 (out),
  3 rewards x 4 choices -> pools 4 / 1 / 6; rerolls = ZIMMERMAN + TIAN-LAO + an empty slot; real clear: offers [4 parts],
  [2 parts] (3rd reward dropped), shop at the clear all empty (every pool part was on offer), grid 4 + 2 tiles, commit
  kept the picks. Everything out (`"SG-027 ZIMMERMAN" = 0`, nothing else listed): pools 0, reroll empty, debug `reward`
  -> [], `choice` -> no grid, real clear: `reward offers []`, no grid, commit (log "WARNING: mission-end commit with no
  reward queued", expected).

## Max occurrences per run (2026-10-29, implemented; replaces [unique_occurrences])
User decisions (2026-10-29): `[max_occurrences] item = N` = offered at most N times per run, counting OFFERS (reward
grid or shop, picked or not), so a core expansion can be raised over several picks. Any part or OS Tuning can be
listed (equipId, part name, OsReinforce row). 0 = no limit (like unlisted); values above 15 are read as 15. A loss on a
failed sortie changes no count (an expansion lost at level 2 with offers left can come back from level 1). A weapon
listed by name covers both its left and right versions, each with its own count (user choice). No alias for the old
[unique_occurrences] section (ignored as an unknown section). Shipped: 5010 = 3, 5020 = 2, 5030 = 3, 5040 = 1 (each
expansion's max level).
- **Storage** (`occur.rs`): an item at its max gets the old "never again" bit (`ROWS_LOST` 4550 + row for lineup
  parts, `OS_GONE` 4418 + rank for OS Tunings), so `candidates` is unchanged for used-up items. Items offered and not
  used up keep their count in a slot table: flags 8030..8991, 74 slots of {9-bit key: lineup row + 1, or 319 + rank
  in OS_TUNINGS; 0 = free; 4-bit count}, LSB first. A slot is freed when its item reaches its max. Table full: the new
  item counts as used up (log warning). `candidates` also skips an item whose stored count already reaches its max (a
  max lowered mid-run).
- **Block 8** (8000..8999): allocated in the shared page array like blocks 4 and 7 (`tools/flag_blocks.py`), 0 flags
  set on a save several missions in (mission 9 area, other session's save) and on saves/ac-mission-available.sl2; no
  8xxx flag in the exported params. 8020 (FLAG_NEVER, the OS Tuning menu lock) is kept out of the range. Not yet
  checked: survives a save + relaunch (the game's autosave), cleared by New Game.
- Marking: unchanged places (5074 for every offer of the sortie, the reroll for shop slots); log
  `max occurrences, offered (<why>): <name> n/max[ (never again)]`; boot log `max occurrences (offers per run): ...`.
  Debug `occur`: count / max / used up per listed item, slots in use.
- **Verified** (profile ac6-roguelike-maxocc.me3, copy of saves/ac-mission-available.sl2, build before #12 was merged;
  config: unlisted_tier 0, ZIMMERMAN + VE-60SNA + 5010 + 5020 at tier 1, ZIMMERMAN = 2, 1 shop weapon): debug
  `reroll` x5 -> ZIMMERMAN row 138 counted 1/2 then 2/2 (never again, slot freed), row 139 1/2 in slot 1 (flags
  8045/8046/8050/8052 = key 140, count 1); `occur` matched. Real clear with `force_os 5010 5020` (1 reward x 3
  choices): offers [Assault Armor, VE-60SNA, ZIMMERMAN 139] -> at 5074 "Bouclier d'assaut 1/3, SG-027 ZIMMERMAN 2/2
  (never again)", then the shop reroll, grid shown. Not verified (user asked to merge without more tests): the pick,
  persistence after a relaunch, New Game clearing block 8, a second offer of an expansion raising it to level 2.

## OS Tunings: one level lost on failure (2026-10-29, implemented)
User decisions (2026-10-29): `[lost_on_failure]` now applies to the 13 non-expansion OS Tunings too; a failed sortie
takes ONE level from each listed installed tuning (not all its levels). Core expansions unchanged (the equipped one:
item + level -> 0). Default config lists all 13 as lost and re-tiers them by effect: rare = build-agnostic power (50
direct hit, 1010 ACS, 2020 repair kits, 5070 weapon bay), common = one weapon type's damage (10/20/30/40) and small
utilities (3010, 4010, 5050, 5060, 5080). The loss dialog gets one line "Niveaux OS perdus : <name> (LV a > b), ..." (the
dialog font has no "→": it showed a box);
an assembly-style loss screen with the shop's chips is a later task (user sketch).
- `penalty::lost_rule` returns the non-expansion rows listed with 1 (`Ctx::os_lost`); `FailurePlan::os_levels` =
  (row, level at sortie) of those with level > 0 (no slot: equipped or not does not apply); `apply_penalty` calls
  `os::level_down` (level - 1, floor 0, direct write like `level_up`). A tuning below its max is rollable again, so a
  lost level can come back as a reward. Unlisted / 0 = kept.
- **Verified** (profile ac6-roguelike-oslevels.me3, fresh copy of saves/ac-mission-available.sl2, config: 3 rewards x 1
  choice, the 10 starter parts and 5050 Boost Kick = 0 in [lost_on_failure]): boot log "OS Tunings losing one level:
  [10, 20, 30, 40, 50, 1010, 2020, 3010, 4010, 5060, 5070, 5080]"; `force_os 10 2020 5050` + real clear -> levels
  0 -> 1 at the commit; kinetic set to 3 by a memory write; escrow 7828 + save, relaunch: plan "OS levels lost [(10, 3),
  (2020, 1)]", kinetic 3 -> 2, repair kits 1 -> 0, Boost Kick kept at 1, dialog line under the part lines, debug `os`:
  kinetic / repair kits rollable again. Second failure (escrow again, relaunch): plan [(10, 2)], 2 -> 1, dialog with
  only "Niveaux OS perdus : Armes cinétiques – Amélioration contrôle de tir (LV 2 > 1)".

## Pools in the settings file (2026-10-29, implemented; replaces the fixed rarity tiers)
User decisions (2026-10-29): one `[pool.NAME]` section per pool (weight, color in hex); `[part_rarity]` keeps assigning
parts, now `part = POOL NAME`; an internal "unlisted" pool always exists and gets every part [part_rarity] does not
list; `= 0` or a pool with weight 0 = never offered (keeps "Parts out of the pools"); the reward grid tiles show the
part's pool name in the pool's colour, on its own line above the category.
- **Settings**: `[pool.NAME]` keys `weight` (0 = out), `color` (`"#RRGGBB"` or `0xRRGGBB`), `show_name` (1 = label on
  the tiles, default 1), `label` (text shown, default the name; always upper case). Pool names match like part names
  (case and spaces ignored). `[pool.unlisted]` is built in (weight 0, grey, no label) and can be tuned.
  `[rarity] unlisted_pool = NAME` sends the unlisted parts to another pool; a name without a section creates a pool
  with the unlisted defaults. `[part_rarity]` values: a pool name, bare or quoted, or 0; an unknown name (or a number
  other than 0: the old 1/2/3 tiers are not read) logs a warning and puts the part in the unlisted pool. Old
  `[rarity]` keys (unlisted_tier, *_weight) are reported as unknown settings. Parser (config.rs): values are numbers
  (decimal, `0x` hex) or strings (quoted or bare), `#` starts a comment only outside quotes; at most 200 pools.
- **Pick** (lib.rs): `Part::pool` = index in `config.pools` (0xFF = `= 0`); `pick_weighted` draws a pool by weight among
  the pools with a candidate left, then a part uniformly in it. Boot log: `pools: unlisted N (weight 0), common N
  (weight 50), ...` and the names of the unlisted parts (40 at most).
- **Shipped pools**: unlisted 0 (#6E7378, hidden), common 50 (#C8CDD2), uncommon 25 (#4FA3FF), rare 15 (#B36BFF), epic
  10 (#FFC23D). `tools/price_model/tier_lists.py` now emits every shop part: weapons by tier-list score (>= 4.5 epic,
  >= 3.75 rare, >= 2.75 uncommon, else common), frames / internals by guide count (3+ epic, 2 rare, 1 uncommon),
  every other EquipmentLineupParam row with a price (rank rewards included: the DLL releases them) common, so the
  unlisted pool is empty (boot: common 201, uncommon 59, rare 37, epic 16, unlisted 0; 313 parts in the pools).
  Lineup rows are named through Smithbox's EquipParam names by equipId: some CSV row names are wrong (60040000 =
  BUERZEL/21D is labelled "IA-C01B: GILLS"). OS Tunings keep the split of "OS Tunings: one level lost" (3 -> epic,
  2 -> rare, 1 -> common).
- **Tile**: the grid movie's tile sprite (900) gets `PoolName` (902, an empty copy of the CategoryName field, one line
  above it; tools/build_rwdchoice.py). The DLL writes the label into `_Content_/TileNN/PoolName` and the colours as a
  4th `SelIdx` field (`sel/count/mask/RRGGBB,,RRGGBB`, empty = no label); the AS3 colours / hides PoolName every frame
  and, when a page has labels, lays the tiles out from the PoolName line down (taller rows, highlight included).
- **Verified** (profile ac6-roguelike-pools.me3, copies of saves/ac-mission-available.sl2, scripted clear): shipped
  config -> 3 offers all "COMMON" (light grey), pick committed. Test config (reward_choices 5, one frame part per
  pool, Boost Kick in `[pool."test pool"]` with `color = 0x40E080`, `label = "très spécial"`, `unlisted_pool =
  leftovers` without a section, SONGBIRDS = legendary, VE-60SNA = 0): boot logged the created pool, the unknown-pool
  warning and pools 1/1/1/1/1, leftovers 306, 2 at 0; the grid showed EPIC (gold), COMMON (grey), TRÈS SPÉCIAL (green,
  above "AMÉLIORATION OS LV 1/1"), RARE (purple), UNCOMMON (blue) on two rows without overlap; the pick (2nd tile)
  was the only part kept at the commit.

## OS Tuning values and own OS chips (2026-10-29, implemented)
User decisions (2026-10-29): `[os_tuning]` table of effect values per level, fewer levels with the same final value;
own chips ("relics") one file per chip, declarative effects now, script hooks (SpEffect...) later; the part params are
NOT edited (they are shared with enemy ACs): every effect applies to the player's AC only, through hooks, and it is fine
that Assemblage does not show it for now. Resupply ("Ravitaillement") and the in-mission cutscenes: later.
- **OsReinforce** (repo index 202, Smithbox def `OsReinforceParam`, row 0xAC): maxLevel u8 +4, iconId u8 +5, 5 levels of
  {addVal s32, exp (OST chip cost) s32, textId s32} from +0x14, 0x18 apart. addVal is `<?osValue?>` in the cat 200 texts
  (221000 "Dégâts cinétiques : +<?osValue?> %"...); repair kits have one text per level (221060..221090, +500..+2 000 per
  kit) and the expansions "Charges +" (addVal 0/1/2). **The game reads addVal at each hit**: the player's level array
  `[GameData+0x1368]` is only read at save load, by the autosave and by the menu's chip sum (exe+48AE30), never at the
  sortie. Verified with the user playing (tester AC mission, kinetic/energy/explosive + ACS at level 1): hits on the
  tester 77 -> 299 when addVal went 3 -> 300 (x1.03 -> x4.00), its hits on the player 66 -> 6 with ACS 3 -> 90.
- `[os_tuning] row = "a/b/c"` (os.rs `apply_values`): levels k < n get the values (cost: the game's, or the last one >
  0), levels above repeat the last value with cost 0 (a save above the new max keeps the max effect), maxLevel = n; text
  = the game's text of the level with the same value (repair kits) else the level's own. ACS (1010) also writes its
  sub-rows 1011/1012 (1013 = coral, all 0, left alone). os.rs `init` applies it once at boot, before reading the max
  levels (the params load once per game start: section "Param edits applied once" below). Shipped:
  10/20/30/1010 = 6/12/15, 40/50 = 10/15, 2020 = 2/4, 3010 = 100. Boot log: `[os_tuning] wrote N field(s)`, max levels.
- **Own chips** (chip.rs, `chips/*.toml` next to the DLL, defaults in dll/src/chips/ written when the folder is
  missing): `id` 1..32 (row 9000 + id in [part_rarity] / [lost_on_failure] / [max_occurrences]; level 2 bits at flags
  4440 + 2 (id-1), used up 4504 + id-1, block 4), `name`, `stat`, `levels` (1..3 values in %), `[effect]` ap /
  en_capacity / weight / ammo = "+"/"-". They are os.rs tunings with `chip = true` (pool, grid tile "Name : PB +5 %",
  level up at the commit, one level lost on failure, occur.rs rank 17 + id-1). Effects: the DLL loop turns the levels
  read when used: each hook (player only) sums its stat's % from the chip level flags at that moment, like the game
  reads its OS Tuning levels at each use, so a pick, a failure, Continue or New Game need no refresh:
  - **max AP**: exe+82AEC0 (rcx = AP block, edx = value; clamps, writes +0x27C, clamps the current) is called every
    frame for every character from 7 call sites; hooked, the value is scaled when rcx is the player's module 0
    ([[[WorldChrMan]+A5A0]+320]+0). A write to +0x27C is undone the next frame.
  - **EN capacity**: exe+88B7C8 calls exe+88BAE0(EN object) every frame and stores the float at +0x20 (current EN at
    +0x24); scaled when the object is the player's module 0x2A.
  - **total weight**: exe+850B77 calls exe+46CA70(assembly) every frame (stored, min 1.0, at module 0x23 +0x408; copy
    at +0x78); scaled when the assembly is PlayerGameData + 0x230.
  - **ammunition**: weapon module = player module 0x27 (vtable exe+3829510, owner character at +8, constructor around
    exe+8FF8AE: 0x20 slots of 0x480 from +0x20; slot equipId +0x44, loaded +0x88, reserve +0x8C, -1 = none).
    exe+907750 adds rounds to a slot (rcx = slot + 0x30, edx = rounds; loaded first, then the reserve capped at the
    weapon's totalBulletNum via the getter exe+15D3110). Called from exe+907661 (filling a weapon, the set-weapon path)
    and exe+907714 (every slot with a rate); a burst at the garage / mission load and for about 5 s after the sortie
    (the game's own: in a vanilla run, DLL off, with a counting cave on both call sites written from Python, ~1 950
    calls in 1 s when the garage opens and ~2 800/s for ~4 s after the sortie, all via exe+907661, none via
    exe+907714; same counts with the mod and the ammo chip at level 0 or 1), nothing later in play. Hooked:
    after the call, a player slot that is full (loaded + reserve = totalBulletNum) gets its reserve raised. Not
    verified: a resupply.
  The part params are not touched (an earlier version scaled EquipParamProtector ap etc.: the tester AC went 12300 ->
  12915 AP, enemy ACs share the frame rows; enemy weapons use EquipParamWeapon_Npc).
- **Verified** (profile ac6-roguelike-osstat.me3, copies of saves/ac-mission-available.sl2): boot max levels
  [(10, 3), (40, 2), (2020, 2), (3010, 1), ... (9001..9004, 3)]; live rows as configured (repair kits texts 221070 /
  221090). All four chips at level 1 (flags set in the garage): player AP 9080 -> 9534 (HUD and AP block), tester AC
  12300 unchanged, EN capacity 2300 -> 2530, total weight 63410 -> 60239.5, ammunition 78/25/140 -> 94/30/168 (tester
  weapons unchanged). `force_os 9001 9002 9004` + scripted clear: grid tiles "Blindage renforcé : PB +5 %  RARE
  AMÉLIORATION OS LV 1/3", picks -> levels 0 -> 1 at the commit, kept after a relaunch; escrow + relaunch: "OS levels
  lost [(9001, 1), (9002, 1), (9004, 1)]", levels 1 -> 0, loss dialog "Niveaux OS perdus : Blindage renforcé (LV 1 >
  0), ...".
- Side fixes: call hooks share cave pages (`alloc_cave`; with ~10 more hooks the near pages ran out and even the shop /
  COAM hooks failed), `alloc_near_page` also searches above the image; trace.rs swallows a hit raised after `unwatch`
  (it crashed the game, exception 0x80000004) and stops its stack walk at an unmapped page (access violation in the
  DLL).
- Test scripts: `garage_to_mission_success.py` skips the mission briefing (double Tab while `menu_drive.on_briefing`
  sees the briefing frame notches at (917..919, 62..64) and (1000..1002, 62..64); no briefing on a second view) and the
  pre-mission talk (double Tab from OUI until the sortie). Garage -> reward grid about 1 min. The "deployment talk" log
  line comes when the mission menu opens, not at the talk. `tools/live_param.py` dumps a live param with its Smithbox
  definition.

## Param edits applied once; frame hook without VirtualQuery (2026-10-29, implemented)
User decision (2026-10-29): the param edits are applied once at boot, with no re-apply check.
- **The game loads its params once per start.** Write watches (trace.rs) on the param repository pointers of
  OsReinforce (`[res+0x80]` holder and `[holder+0x80]` file), MissionParam and TalkParam (`res` = `[repo + 0x80 + index
  x 0x48 + 8]`, repo `[exe+4E27300]`), armed in the garage: no write through garage -> tester mission -> results ->
  garage, then system menu -> Retour à l'écran-titre -> Continue. The OsReinforce data sits at one of two addresses
  (0x7FF4F92xxxxx or 0x7FF4D3Exxxxx) depending on the launch: that, not a reload, is the "other copy" seen in the OS
  Tuning values work. The only reload the game could do is an online regulation download (title: "Version
  réglementation 1.09.1"), not reproducible offline: the edits would then be lost until the next start.
- Removed: the 5 s re-apply of the economy, the OS Tuning menu lock and the ALLMIND line cut, and the per-tick
  `[os_tuning]` stamp check (`param_stamp`, `param_index_cached`). The 5 s flag page cache refresh stays.
- `penalty::frame_hook` (game thread, every frame) used `game_data()` (3 VirtualQuery) before checking for work: now
  `game_data_fast()`, raw reads of GameDataMan (lives as long as the game) and its player GameData pointer (0 until a
  save is loaded).
- **Verified** (profile ac6-roguelike-osstat.me3, copy of saves/ac-mission-available.sl2): boot log `[os_tuning] wrote
  75 field(s)`, `economy: 220 reward fields scaled at boot`, menu locked, ALLMIND lines silenced; after a full mission
  OsReinforce row 10 still 6/12 (max 3), 2020 2/4 (max 2); reward grid pages, picks and commit through the frame hook;
  a failure (escrow + relaunch) applied its penalty and opened the loss dialog.

## Modular chips: architecture and roadmap (decided with the user 2026-10-29)
Goal: new content is files (declarations) plus, when it needs logic, a script; the DLL is a fixed engine.
- **Engine (DLL)**: *modifier targets* (named values looked up where they are used: `chip::Target`, `chip::modify`),
  *events* (mission start, cleared, failure, reward roll, shop reroll, pick, save loaded; later hit, kill, resupply:
  each one an init point or hook, never a poller) and *actions* (change a modifier, apply a SpEffect to the player,
  grant an item / credits, set a flag, offer a reward).
- **Content (files)**: chips (`chips/*.toml`, `[effect]` target = mode), later other kinds (event cards, mission
  modifiers); game data files (param edits, new rows: OsReinforce values, mission pay, new SpEffectParam rows) compiled
  by a build tool into a `regulation.bin` in the mod package (the only way to add rows; the DLL would stop patching
  tables at boot).
- **Scripts**: an embedded Lua (mlua, vendored 5.4) or Rhai, one optional script per content file, handlers per event
  calling the actions.
- Steps: (1) modifier targets [done, below], (2) events + actions [done, below], (3) regulation generator [done,
  below], (4) SpEffect RE + `speffect` action [done, below], (5) script engine [done, below].

## Modifier targets (2026-10-29, implemented; step 1 of the roadmap)
- `[effect]` keys are targets, values a mode: `"+"` / `"-"` (the level's value in %), `"add"` / `"sub"` (the value
  itself). `chip::modify(target, v)` = v x (100 + % sum) / 100 + flat sum (>= 0), the levels read from the flags when
  the target is used (several chips add up). Tile: "Name : Stat +v %" (no % for add / sub).
- Targets: `ap`, `en_capacity`, `weight` (min 1.0), `ammo` (player hooks, section "OS Tuning values and own OS chips");
  `pool.<NAME>.weight` (lib.rs `pool_weight`, every pool lookup of the shop and reward rolls: in_pool, pick_weighted);
  `reward.count` (0..3) and `reward.choices` (1..12) in `roll_rewards` (sortie); `shop.weapons` / `shop.parts` in
  `pick_shop` (total MAX_SHOP_SLOTS). A target named in no chip costs one scan of the chip list.
- Default chip 9005 "Contrat préférentiel": `"pool.epic.weight" = "+"`, 25/50/100, pool epic, lost on failure.
- Debug `odds [N]`: N weighted picks among the reward candidates, share per pool and its weight after the chips.
- **Verified** (profile ac6-roguelike-osstat.me3): `odds 20000` common 50.0 / uncommon 25.1 / rare 15.1 / epic 9.7 %,
  then with 9005 at LV3 (flags 4448/4449) epic weight 20 -> 17.8 %; a test chip (`reward.choices`, `reward.count`,
  `shop.weapons` = "add", LV2): debug `reward` 3 offers of 5 parts, `reroll` 5 shop slots (3 weapons + 2 parts), real
  clear: grid 3 pages of 5 tiles.

## Advanced chip design (example, not implemented)
"Surcharge de combat": each enemy destroyed during a mission raises the AC's damage (stacking), at a fixed AP cost.
Shows the three layers working together:
```toml
id = 7
name = "Surcharge de combat"
stat = "Dégâts par élimination"
levels = "3/5/8"                 # % per kill, the chip's levels
script = "combat_overload.lua"   # optional logic (step 5)

[effect]
ap = "-10%"                      # fixed value at every level (format extension: explicit values per effect)

[speffect.attack_up]             # a new SpEffectParam row, compiled into regulation.bin (step 3)
# fields by Smithbox name; {level} = the chip's level value, the row id chosen by the builder
physicsAttackPowerRate = "1 + {level}/100"
magicAttackPowerRate = "1 + {level}/100"
effectEndurance = -1             # until removed
```
```lua
-- combat_overload.lua: engine events call these; ctx = the engine's actions (step 2 / 4 / 5)
local stacks = 0
function on_mission_start(ctx) stacks = 0 end
function on_enemy_destroyed(ctx, enemy)
  if stacks < 5 then
    stacks = stacks + 1
    ctx.apply_speffect("attack_up")   -- the row above, on the player only
  end
end
```
Engine needs for it: the `on_enemy_destroyed` event (RE: a character's death, owner = enemy), `apply_speffect` on the
player (SpEffect RE, step 4; stacking = one row applied n times or n rows), the regulation builder for the new row
(step 3), the script host (step 5); the fixed `ap = "-10%"` is a small parser change on top of step 1.

## Events and actions (2026-10-29, implemented; step 2 of the roadmap)
User decisions (2026-10-29): actions credits, give, level, flag; as many events as the existing detections give.
- `event.rs`: `emit(Event)` at the points the mod already detects, each chip at level 1+ subscribed with an
  `[on.<event>]` section runs its actions. Events and where they are emitted:
  `save_loaded` (flag 1000 rising, live save), `title` (flag 1000 falling), `garage_opened` (`on_garage_open`: save
  loaded or back from a failed mission), `deployment` (escrow set during the pre-mission talk, flag 5003),
  `mission_start` (defer 0 -> 1), `death` / `respawn` (player AP 0 / back above 0), `mission_cleared` (5074 rising
  after a sortie), `results` (rewardmenu results hook, the grid opens), `commit` (defer 1 -> 0, save loaded),
  `mission_failed` (penalty applied, game thread), `game_over` (dead save at load, unaffordable failure),
  `shop_reroll` (`reroll`), `pick` (each picked offer at the commit, game thread), `part_sold` (Vente sale hook, game
  thread), `auto_sell` (frame hook).
- Actions: `credits = N | "a/b/c" | "value"` (COAM, the chip's level value with "value"), `give = "part name" or
  equipId` (one copy through the game's give call; a booster / generator / FCS only if not owned), `"level.<row or
  name>" = "+1"` (os::level_up / level_down, OS Tunings and chips), `"flag.<id>" = 1 / 0`. flag / level act at once;
  credits / give are queued and applied by the frame hook in the garage (game thread, like the penalty), their lines
  ("Prime de performance : +4 000 COAM", "Pièce obtenue : ...") shown in one game dialog once the garage menu is up.
- **Garage menu on screen**: the msg hook counts lookups of its title (cat 200 id 100000 "GARAGE"); it is looked up
  when the garage menu is created (after a load, and after a mission once the notification and talk are over), not
  every frame. A dialog opened earlier (the menu system is up during the results -> garage load) is lost.
- `dialog::show` while a dialog is still requested adds its text under the pending one (one dialog).
- Chip file keys: `unit` (tile text after the value, default " %" or nothing for add / sub effects and action-only
  chips). Default chip 9006 "Prime de performance": `[on.mission_cleared] credits = "value"`, 2000/4000/8000, unit
  " COAM", pool rare, lost on failure.
- **Verified** (profile ac6-roguelike-osstat.me3): a test chip with flag actions on 9 events + give / level on commit,
  and 9006 at LV2: log order deployment, mission_start, mission_cleared (credits), shop_reroll, results, pick, commit
  (flag, give SG-027 ZIMMERMAN, level 9001 0 -> 1); COAM 58 906 -> 62 906 at the commit; after the post-mission talk
  the dialog "Prime de performance : +4000 COAM" over the garage menu (title lookups 2 -> 3). save_loaded /
  garage_opened not exercised (levels set after the load).

## Regulation generator (2026-10-29, implemented; step 3 of the roadmap)
Decision (2026-10-29, "keep going" on the proposal): a build step, not the DLL; only new declarations go through it
for now (the boot-time edits of [os_tuning], the economy, the menu lock and the ALLMIND lines stay in the DLL).
- `tools/regulation/` (C#, .NET 10) on Smithbox's Andre.SoulsFormats (`SFUtil.DecryptAC6Regulation` /
  `EncryptAC6Regulation`, BND4, PARAM, PARAMDEF): reads the game's `Game\regulation.bin` (AES, then DCX
  `DCX_DFLT_11000_44_9_15` = zlib: no Oodle needed; 257 params), applies `data/regulation/*.toml`, writes
  `mod/regulation.bin` (re-encrypted, same DCX). `tools/build_regulation.ps1` builds and runs it (SMITHBOX_DIR or the
  default Smithbox path); `tools/package.ps1` ships the file when there are declarations. Without declarations
  nothing is written (an old file is removed).
- Declarations: `[ParamName.rowId]` (regulation file name) + `field = value` (Smithbox field name, case ignored);
  a missing row is added, `copy = <rowId>` clones a row first; rows re-sorted by id (the game binary-searches them).
  Definitions: `Name.xml`, `NameParam.xml` or the name without "Param" (SpEffectParam -> `SpEffect.xml`); AC6 PARAM
  files carry no ParamType.
- **me3 serves `mod/regulation.bin`** from the profile's package (no extra setting).
- **Verified**: TURNER `totalBulletNum` 540 -> 777 and a new SpEffectParam row 900000001 (copy of 6200,
  `effectEndurance` 12.5) live in game (`live_param.py`; SpEffectParam 4121 rows), a full mission played normally.
- The file carries the whole game regulation: rebuild after a game update (else the old data replaces the new).

## SpEffects on the player (2026-10-29, implemented; step 4 of the roadmap)
- **Repair kits** are SpEffectParam 9403..9407 (`changeHpPoint` -4000..-6000, instant: 4 000 AP + 500 per Repair Kits
  tuning level; the Paramdex names 9303..9307 are outdated). Keyboard: C uses one.
- **SpEffect manager** of a character: `[chr + 0x308]` (vtable exe+3839EB8): +8 head of the active list, +0x10 owner
  character. Entries: +0 id (and a byte at +4), +8 the param row, +0x38 next, +0x64 flags (processing loops at
  exe+966C00 / exe+96A420; an instant effect is linked, processed at exe+96E550 (reads `changeHpPoint` +0xA4) and
  unlinked the same frame).
- **Apply**: found by a write watch on the last entry's next pointer during a repair: the insert exe+969FE1 in
  exe+969F70 (allocate + link at the tail), called by exe+96B6C0 (manager, id, ...), wrapped by **exe+968EA0(manager,
  id, target chr, source chr, *params, u8, u8, u8)**; its 6 callers pass the character's own manager, the character as
  target, a source, a pointer to 4 floats (1.0 where filled) and 0 flags. Game thread only.
- DLL: `event::queue_speffect` + `apply_speffects` (penalty.rs frame hook): the player is target and source, params
  4 x 1.0, flags 0; the manager checked by its vtable. A SpEffect queued at `mission_start` waits for the mission's
  player character (the garage has its own player character at the sortie). Chip action `speffect = <row>` or
  "a/b/c" (row per level); debug `speffect <id>`.
- Chip files: `[speffect.<row id>]` sections = SpEffectParam rows compiled by tools/regulation (RegBuild reads
  `dll/src/chips` too); the DLL ignores those sections.
- Default chip 9007 "Nanoréparation": rows 900000071..073 (copy of 9405, `changeHpPoint` -20/-40/-60,
  `motionInterval` 1.0, `effectEndurance` -1) applied at mission_start: +20/40/60 AP per second (row 1 of the game
  is such a 10 AP/s regeneration). Pool epic, lost on failure.
- **Verified**: debug `speffect 9405` with AP 3000 -> 8000; a generated row (copy 9405, `maxHpRate` 1.2,
  `effectEndurance` -1) -> player max AP 9080 -> 10896, tester AC unchanged; Nanoréparation LV2: applied 9 s after the
  sortie to the mission character, AP 5000 -> 5240 in about 6 s.

## DLL-boot regulation: fallback path (2026-10-29, feasibility checked once; not implemented)
The regulation tool (tools/regulation -> mod/regulation.bin) stays the way rows are added. This section keeps a
switch easy if it ever causes trouble (Smithbox dependency, game update): the DLL would build the same rows once at
boot instead. Nothing here runs at play time; the check below was a one-off script against a running game.
- **Where the file lives**: repository slot `[exe+4E27300] + 0x80 + idx*0x48`, res at slot+8, holder `[res+0x80]`:
  `holder+0x78` u64 file size, `holder+0x80` file pointer (SpEffectParam: index 30, 4123 rows, row size 0x3BC,
  header byte +0x2D = 0x85, +0x2E = 7, data from +0x182C8, strings at the end, every row name = the same empty string).
- **Hidden index around the file** (same allocation, not part of the PARAM format): at file-0x10
  `{u32 file size, u32 row count, u64 ptr (kept as is)}`, and after the file, at `file + align16(size)`, `count`
  entries `{u32 id, u32 row index}` sorted by id (unsigned), then `{FFFFFFFF, count}`. The row lookup (e.g. the
  SpEffect one at exe+15CA0A3: `GetParam(repo, 0x1E)` exe+15C8980, then binary search over that index, then the row
  table's data offset at `file + 0x48 + idx*0x18`) never reads the PARAM row table for the search.
- **First try crashed** (AV at exe+15CA0D2 reading file-0x10): a bare file without the prefix and index.
- **Verified swap** (scratchpad spswap.py, at the title screen): new block = prefix (new size, new count, old ptr) +
  file (header: +0 strings offset, +0x10 same, +0xA row count, +0x30 data start; row table with the new row
  inserted at its sorted place, offsets shifted; old data + new row appended; strings) + padding to 16 + index +
  sentinel; `holder+0x78` = new size, `holder+0x80` = new file. Then Continue, garage: debug `speffect 900000081`
  (new row) -> ret 1 and an active entry whose row pointer is the new row; `speffect 900000082` (absent) -> ret -1;
  vanilla 9405 -> ret 2; the game kept running. The old block stays allocated (never freed by the game: params load
  once per boot).
- **Plan for the DLL**: at boot, once the repository is filled (where `os::apply_values` runs today), for each param
  with declared rows: read the live file, merge the rows (new ids inserted sorted, existing ids overwritten in place),
  allocate the block (VirtualAlloc, read/write), write prefix + file + index as above, repoint the holder. Row fields
  come from the declarations by field name, so the DLL needs the field layout: generate a Rust table from the
  Smithbox paramdefs at build time (only for the params we declare: SpEffectParam first), or ship small layout files.
  Once that works, RegBuild and mod/regulation.bin go away (and with them the regulation encryption step).

## Chip scripts: Lua engine (2026-10-29, implemented; step 5 of the roadmap)
- **Design (user)**: Lua 5.4 (mlua 0.10, vendored, in the DLL); a chip's script is a separate file next to its .toml
  (chips/<name>.lua); safe API for every script, raw memory only in an `unsafe` context (`unsafe = true` in the chip
  file gives the script the `unsafe` table).
- **Runtime** (script.rs): one Lua state on the game thread, built at the first call (penalty.rs frame hook,
  `script::run_queue` before the SpEffects). Libraries: table, string, math, utf8 (no io / os / package / debug;
  dofile / loadfile / load / require / collectgarbage / print removed). Each chip runs in its own environment table
  (globals through `__index`); its `on_<event>(e)` functions are found at load by name (chip.rs `script_on`) and queued
  by `event::emit_with` with the chip's level at the event (`e.level`, `e.value`, event fields), then run the next
  frame. A call is stopped after 2 000 000 instructions (count hook); any error is logged
  (`script <chip>: on_x failed: ...`) and the game goes on.
- **API**: `log(...)`; `chip` (.id, .name, .values, level(), value()); `store.get()/set(v)`: one byte per chip saved
  with the run (flags 7132 + 8 x (id - 1), cleared when the chip falls to level 0); `game` (ap(), max_ap(), coam(),
  in_mission(), flag(id), level(row or name)); actions = the declarative ones: credits(n), give(name), speffect(row),
  add_level(row or name, n), set_flag(id, bool) (a `speffect` from `mission_start` waits for the mission's AC);
  `unsafe` (base, player(), readable(addr, n), read_/write_ u8 u16 i32 u32 u64 f32; an unmapped address reads nil and
  is not written).
- **panic = "unwind"** in the release profile: mlua raises Lua errors (any script error, the instruction budget) by
  unwinding; with "abort" the first script error killed the game (fast fail c0000409 in the DLL; reproduced in a test
  crate: nil index, error(), an error from Rust and the budget stop all abort; with unwind all four come back as
  errors).
- **mission_failed** is now emitted before the penalty (the chips it takes were installed during the failed mission).
- **Verified in game** (osstat save, tester AC mission): the three default scripted chips below; the unsafe API (base,
  player pointer, a bad read = nil); an endless loop stopped ("stopped after 2000000 instructions") with the game
  running.

## Enemy destroyed event (2026-10-29)
- Every character's update (exe+756370, every frame) compares its AP ratio with the last frame's (AP block +0x388
  ratio, +0x38C AP) and calls **exe+1714F00(chr)** at **exe+7563FF** when the ratio moved by more than 10 % or the AP
  changed and is below 10; the last-frame values are written after the call. A destroyed character always goes
  through it once (AP > 0 -> 0, also when the AP is zeroed from outside). The DLL hooks that call: AP 0 now, above 0
  last frame, not the player -> `enemy_destroyed` (`e.max_ap`). Installed only when a chip uses the event.
- Dead ends: the per-frame readers of AP +0x278 (exe+706AFC, exe+82B73A "alive" checks, the HUD gauge writer
  exe+730109 = AP block +0xE8 segment count), module 0x2B's ring of animation events (id 250 queued at the death,
  generic TAE event code exe+A69DA3), a diff of the ChrIns itself (no change within 1 s of the death).

## Scripted default chips (2026-10-29)
- 9008 **Prime de série** (streak_bonus.lua): every 3rd mission cleared in a row pays 10 000 / 20 000 / 40 000 COAM;
  a failed mission resets the streak (store byte). Verified: streak 1/3 after a clear; streak at 2 + a clear -> "Prime
  de série : +10 000 COAM" dialog, COAM 58 906 -> 68 906, streak 0; a failure with the chip lost -> "streak reset" and
  the byte cleared.
- 9009 **Récupération** (salvage.lua): each enemy destroyed applies row 900000091..093 (copy of 9405,
  `changeHpPoint` -300 / -600 / -1000). Verified: tester AC zeroed with the player at 5000 AP -> one "enemy destroyed"
  and AP 5300.
- 9010 **Dernier recours** (last_stand.lua): at mission_start applies row 900000101..103 (`effectEndurance` -1,
  `conditionHp` 30, the four `*AttackPowerRate` 1.1 / 1.2 / 1.3). The game evaluates the condition itself: the active
  entry's state (+0x60) is 1 above 30 % AP and 4 at 30 % or below, switching both ways (AP 9080 / 2000 / 5000 / 2700
  tested). The damage gain itself was not measured.

## Inventory cap (2026-10-30, implemented)
User decisions (2026-10-28 / 30): every weapon / frame part / internal owned counts, equipped ones included, each copy one
(core expansions do not); `[inventory] max_parts = 20` (at least 7 = the frame parts / internals of an AC; weapon slots may stay empty); a shop purchase
while full is BLOCKED; ANY part entering the inventory over the cap shows a "full" dialog, then the Vente list, again
each time it closes while still over (no auto-sell); with several rewards the check runs between the grid pages.
- **Count** (`cap.rs`): inventory entries (penalty.rs table walk) with tag 0 (not 75xxxxxx), 1, 5, 6, 7.
- **Single entry point = the table inserts.** AddItem exe+474D80 is NOT the only way in: the commit of the obtained
  items (exe+4756E0) inserts its elements itself. Both end in **exe+C2A0F0 / exe+C2A180** (one per half of the
  inventory table; rcx = item manager + 8, rdx = item desc; index or -1); their only callers are AddItem (474E91 /
  474EA2: give, purchase, starters bought after a failure, chip gives) and the commit (4757A4 / 4757AB). Verified with
  breakpoints: a debug give hits 474D80 then C2A180 (from 474EA7), the reward commit hits C2A180 from 4757B0 only.
  Queued (deferred) gives return before the insert. The DLL hooks the 4 call sites: an AddItem insert asks for a check
  on the next garage frame; a commit insert waits for the garage (below).
- **Purchase block.** The shop's buy step exe+D66940 calls `exe+D2F860(const u32* id)` (= D2F900 on a copy) at
  exe+D669C5; 0 -> the game's own refusal dialog with message cat 204 id 231020 ("Vous avez acheté toutes les pièces
  disponibles."), before the confirmation and the price check. Hooked at that call site: full -> 0. The message is
  looked up BEFORE the check runs, so msg.rs serves "Inventaire plein (N pièces maximum). Vendez une pièce pour en
  acheter une autre." whenever the inventory is full. Buy-back and starters go through the same step.
- **The Vente list from the DLL.** Garage MAGASIN DE PIÈCES -> ACHAT / VENTE is a submenu of the garage menu (no new
  job); confirming VENTE makes the menu manager run a std::function (exe+D74BC0: `call [vtable+0x10]` at exe+D74BF0)
  whose callable is the game's function-pointer wrapper (vtable exe+3912780, pointer at +8) around
  **exe+10A5C00(out RefPtr<MenuJob>, ctx)**. That factory calls the builder exe+10A6F20 with mode 0x23 (the Achat
  factory, around exe+10A5B7A, passes 0x22), which makes the shared part-list window `02_010_EquipTop` (class vtable
  exe+39E9F68, ctor exe+10A2E60; Assemblage uses it too).
- **Menu manager: a queue and ONE child slot.** `exe+DA3410(&job, &f)` + `exe+DC2A70(&job)` hands a job to the manager
  queue (exe+D834E0): it waits until the current modal menu is done (over the reward grid it only opened after the
  grid). **exe+DC4B60(out handle[3], &f)** (what the generic dialog builder exe+DC1650 calls) makes the job through
  exe+D74BC0 (ctx = manager+0x10) and opens it at once as the manager's child (exe+D83350; current child +0x68/+0x70,
  next +0x78/+0x80); **exe+DC4AE0(job, id)** = 1 while that job is the current or next child. Opening a second child
  closes the first: dialog + Vente opened together replaced each other in a loop (the list close hook re-prompted at
  every replacement). Now: the dialog first, and Vente (DC4B60 + the function-pointer callable) once DC4AE0 says the
  dialog is gone (checked only while our dialog is up). Child windows of a menu (Vente's sell confirmation) draw over
  everything; the Vente list itself draws UNDER the reward grid movie (layer from the menu table flags?, not checked).
- **Close event.** Vtable slot 1 of the list window (exe+39E9F68+8) is its deleting destructor exe+10A58C0(this,
  flags); the DLL swaps the slot: while over the cap, a closing list window (Vente, or Achat / Assemblage) re-runs the
  check (dialog + Vente again).
- **When the check runs.** Save load (flag 1000 edge), any AddItem insert in the garage, a list window closing while
  over, a grid page after the first (results), and after a mission commit. Not AT the commit: menus pushed then are
  dropped when the results screen is torn down. After the commit flag 3139 rises (~4 s) for the post-mission garage
  sequence (Walter's talk) and falls when the garage menu is back: the commit's check runs on that fall (a sortie or a
  save load clears the wait, in case a commit has no 3139 sequence). OpenSession is not called around the tester-AC
  mission (only at save load).
- **Between grid pages.** Rewards are all committed at the end; each page after the first counts the parts picked on
  the pages before it (+ the inventory) and, over the cap, opens the dialog then Vente over the results. The grid
  hides itself meanwhile (5th SelIdx field "hidden" -> the movie sets `visible = false`, the grid ignores the menu
  input: else our dialog's OK confirmed the grid page, and Vente was drawn under it) and comes back when the list
  closes. The results table shows through behind Vente (cosmetic). The last page's pick is checked after the commit.
- **Nothing to sell** (counted parts all equipped or priced 0, e.g. the Worm Killer): the dialog says to take parts off
  the AC in ASSEMBLAGE and sell them; no Vente (it would reopen at once forever); the check runs again when a list
  window closes. Not reproduced in game (needs an unsellable spare).
- **Verified in game** (profile ac6-roguelike-cap.me3, copy of saves/ac-mission-available.sl2, 17 parts):
  max 16 at load -> dialog -> Vente; Retour without selling -> dialog + Vente again; a sale -> 16/16, Retour ->
  garage. Max 17: buy-back at 16/17 bought, then at 17/17 refused with the new text, credits unchanged. 2 rewards per
  mission at 17/17: page 2 opens -> dialog over the results (grid hidden) -> Vente -> TURNER sold (+13 750) -> grid
  back -> reward 2 picked -> after the talk "18 pièces pour 17 places" -> Vente; Retour -> dialog again. Max 18:
  debug give2 -> 18/18 silent, -> 19/18 dialog, another give with the dialog up -> "20 pièces ... vendez 2 pièces",
  one OK -> Vente.
- Not covered: arena rewards (no arena on this save: is there a 3139 sequence after an arena commit?), chip `give`
  actions (same AddItem path, not run), mouse clicks on the hidden grid's OK while Vente is open, reward_choices = 1
  without OS offers (no grid: only the after-commit check). After Vente closes, the vanilla VENTE tutorial sometimes
  pops up (cause not checked; tutorial texts: separate task). Breakpoints on per-frame menu functions (exe+DC2A70,
  exe+D834E0) froze the UI: watch only functions that run once per menu.
