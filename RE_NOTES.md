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
4. **Pool without repeats (TBOI-style)** — replaces progressive rarity odds.
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
| `GameData+0x230` | id set used by the NEW-badge predicate (probably "seen in shop") |
| `exe+434FE0..4352xx` (start = prologue `mov [rsp+8],rcx; push rbp..r15; sub rsp,108`) | `CSEquipmentLineupData` ctor/init: caches all lineup rows into std::maps (MSVC node: `+19 isnil`, `+1C key`), incl. a map keyed by release flag. Reads `eventFlag_forRelease` at `exe+4352D6`. Runs on title-screen transitions and Continue, **not** on mission/arena return. Source file string: `..\..\Source\GameData\CSEquipmentLineupData` |
| `exe+435B50` / read at `+435B86` | lineup price getter (`mov eax,[rax+4]`) |
| `exe+435C90` | copies `item[+0x54]` (gaitem id) to `item[0]`, returns &item |
| `exe+B885CF` | `CSGaitemAccessor::GetPrice`-like: dispatch on id top nibble (`shr 0x1C`), parts → `[GameDataMan]+0x40` → `+435B50` |
| `exe+E87F80` | shop menu per-frame function: `+E87FD8 call +E88180(menuObj, &handle)` → RAX = handle; `+E87FE9 call +E928F0(list, ?, &filter)`. **`+E88180` rebuilds the whole list every frame** (see Answer) |
| `exe+E88180` | `jmp exe+641901C` (Arxan-relocated body). Fills a stack handle; `[handle+0]` = persistent container |
| container (`7FF39A7A33F0` last session) | `+0 vtable exe+395CE18`, `+8 refcount`, `+10 vtable`, `+18 begin`, `+20 end`, `+28 cap` |
| shop list entries | stride **0x118**, 259 entries. `+10 vtable exe+390BA08`, `+20 price`, `+28 shared ptr`, `+50 int`, `+54 gaitem id` (`0x10000000 \| equipId`), `+58 ordinal`, `+60` ptr to a 0x20-stride **stats param row (not the lineup row; shared by the ±0x100 variants)**, `+68/+6C/+70 ints`, `+74 byte` |
| `exe+E928F0` | per-frame loop: `for i<count: item=src[i]; if filter->Accept(item) push(item)` (`+E929B4 call [rax+10]`) |
| `exe+E88B10` | Accept trampoline: `jmp [filterObj+8]` |
| `exe+E89820` | NEW-badge predicate: `+D2F860(+435C90(item)) > 0` — returned 0 for every entry (all seen) |
| `exe+D2F900` | `1 if +474060(GameData+0x230, &id) < 0` (id not in set) |
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
for later: 4418..4549, 4868..4899, 4901..4999, 7418..7499, 7501..7799, 7838..7899, 7945..7999. Helpers on `Game`:
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
