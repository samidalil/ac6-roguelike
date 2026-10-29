# ac6_roguelike — me3 native DLL

Roguelike mode for Armored Core VI 1.09 (Steam). Ships no `regulation.bin`: params are edited in memory, game code
is hooked with small call-site caves. All addresses live in `src/offsets.rs` and are documented in `../RE_NOTES.md`.

## Build
Needs the Rust MSVC toolchain (`x86_64-pc-windows-msvc`). Close the game first: it locks the DLL.
```
cargo build --release
```
Output: `target/release/ac6_roguelike.dll`, loaded by the `[[natives]]` entry of `../ac6-roguelike-fresh3.me3`.
The reward choice screen is a menu movie, `../mod/menu/02_rwdchoice.gfx` (loaded by the profile's `[[packages]]`
entry), built from the game's own 02_gotreward.gfx and `../mod/src/rwdchoice_MainTimeline.as` with
`powershell -File ../tools/build_rwdchoice.ps1` (needs FFDec + Java, see the script).
Release package (DLL + mod folder + profile + README + zip in `../dist`): `powershell -File ../tools/package.ps1`.

## Run
```
me3 launch --auto-detect -p ../ac6-roguelike-fresh3.me3
```
Log: `%USERPROFILE%\ac6_roguelike_dll.log` (rewritten at every launch).
Settings: `ac6_roguelike.toml` next to the DLL, written from `src/default_config.toml` when missing.

## Modules
- `lib.rs` — boot (config, param lookup, hooks), the shop (candidates, pick by `[pool.NAME]` weight then uniform in the
  pool, `= 0` / weight 0 parts out of the pools, pool sizes and unlisted parts logged at boot, reroll every `reroll_interval` cleared
  missions, state kept in event flags), saved state helpers (flags, multi-bit values, row bitsets one flag per lineup
  row: `read_rows` / `write_rows` / `clear_rows` / `put_row` / `has_row` over `o::ROWS_*`), economy (mission pay, first mission = starting credits), mission rewards (queued at mission start through the game's give
  path), the game-thread loop and the game-over exit sequence.
- `penalty.rs` — failure plan built at sortie (escrow flag), penalty on the next garage load, auto-sell, game over,
  part values (shop price, stat model, `[prices]` overrides) and the sell-price hook. `[lost_on_failure]` decides per
  item whether the equipped part is lost (unlisted: weapons / frame / internals yes, OS Tunings no); a lost core
  expansion (assembly slot 11, `+0x520`) loses its item and its OS Tuning level, a listed non-expansion OS Tuning
  loses one level (`os::level_down`), shown in the loss dialog.
- `buyback.rs` — parts sold since the last reroll (Vente sale hook + auto-sales) offered again in the shop, kept in
  the `ROWS_SOLD` bitset, cleared at the reroll.
- `choice.rs` — reward choice (pick 1 of `reward_choices` per reward): offers rolled at sortie with their own RNG
  (saved in event flags), queued like plain rewards; at the commit the parts not picked leave the pending list.
- `dupshop.rs` — duplicate parts in the shop: weapons / frame parts can roll while owned; the hook on the shop's
  "obtainable" check (exe+D2F900) lists and sells a slot's part while the player owns no more copies than when it
  was rolled (per-slot baselines in event flags). Also clears the old "already rolled" flag ranges once per save.
- `os.rs` — OS Tunings as rewards (never in the shop, offered while below their max level): levels at
  `[GameData+0x1368]`, a pick adds one at the commit (choice.rs); a core expansion not owned also queues its item,
  other offers queue a carrier item taken out at the commit; the vanilla OS Tuning menu stays locked (its unlock row
  points at a never-set flag, the unlock notification no longer names it). `[os_tuning]` rewrites a tuning's effect
  value per level in the OsReinforce param (the game reads it at each hit; fewer levels = lower max level).
- `chip.rs` — our own OS chips: one file per chip in `chips/` next to the DLL (`id`, `name`, `stat`, `levels`,
  `[effect]` ap / en_capacity / weight / ammo = "+" / "-"), offered and leveled like OS Tunings (rows 9000 + id; levels
  in flags 4440..4503, used up 4504..4535). Effects on the player's AC only, through call hooks on the game's stat code:
  set max AP (exe+82AEC0), EN capacity (exe+88BAE0), total weight (exe+46CA70), add ammunition (exe+907750); the part
  params are untouched. Effects are modifier targets (`Target`, `modify`): the player's stats above, plus the run's own
  values looked up where they are used (pool weights, rewards per mission, parts per reward, shop size).
- `occur.rs` — `[max_occurrences]`: offers per run of the listed items (any part or OS Tuning), counted at the mission
  clear (rewards) or the reroll (shop) in a slot table (flags 8030..8991, 74 slots of {item, count}); at its max an
  item gets its "never again" bit in `ROWS_LOST` (lineup parts) or `OS_GONE` (flags 4418..4434, OS Tunings).
- `rewardmenu.rs` — the choice grid: the results screen's "Pièces obtenues" call opens our movie instead (one page per
  reward, tiles filled by the game's own part view, each with its pool's label in the pool's colour: `PoolName` text,
  colours in the `SelIdx` state), navigation from the game's menu actions (pad + keyboard).
- `trace.rs` — RE tool: hardware breakpoints + VEH log (debug `watch` command, `%USERPROFILE%\ac6_watch.txt`).
  Call hooks share cave pages (`alloc_cave`): one 64 KB allocation slot near the image per page, not per hook.
- `config.rs` / `default_config.toml` — settings file parser and defaults (economy, rewards, shop, pools:
  `[pool.NAME]` sections + the built-in unlisted pool, `[part_rarity]` part = pool, prices, per-item tables `[lost_on_failure]` / `[max_occurrences]` read through `config::lookup` / `config::value`, `[os_tuning]` values per level).
- `dialog.rs` — the game's OK dialog, opened with our own text; `msg.rs` — message lookup hook (custom texts).
- `offsets.rs`, `mem.rs` — addresses, memory helpers (logging is in `lib.rs`).

## Status
Playable end to end: mission pay removed, random shop and rewards from pools defined in the settings file (pick 1 of 3 per reward, each tile showing
its pool in the pool's colour; weapons
and frame parts can repeat and be owned several times, boosters / generators / FCS once; OS Tunings, core
expansions included, are rewards only, one level per pick, the OS Tuning menu locked; core expansions offered as
many times per run as their levels and lost on a failure; OS Tuning values per level in the settings; our own OS
chips from `chips/` files: AP, EN capacity, weight, ammunition, on the player only), failed sorties take parts,
auto-sell
when broke, game over back to the title with the run's save closed. Vanilla rank / arena reward parts are not granted;
they are shop parts like the others.
