# tools/ — CE-free RE helpers (Python 3.11 + capstone)

Run from this folder with the game running (`PYTHONIOENCODING=utf-8 python <script>`; set `AC6_PID` if several instances).

- `ac6_lib.py` — ReadProcessMemory wrapper, module image, capstone disassembly, `callers()`, rip-relative annotation.
- `flag_test.py` — finds the live shop-list container (vtable exe+395CE18), reads a shop entry's release flag via the
  reimplemented CSEventFlagMan bit lookup, clears it, re-reads the list, restores it. **Needs the Parts Shop open.**
- `flag_blocks.py` — lists CSEventFlagMan's blocks (id // 1000) with their set flags; `--show`, `--snap`/`--diff`,
  `--set ID VAL…` (direct bit write + dirty byte, like the DLL), `--save` (requests a save). Used to pick and prove the
  saved flag ranges of the mod's state.
- `lineup_dump.py` — parses the 318 live `EquipmentLineupParam` rows (data at type-string − 318·0x20) and prints
  flag/price/reward populations and the rows behind the `CSEquipmentLineupData` +0x08 bool map.
- `flagman_xrefs.py` — confirms the CSEventFlagMan instance/global, lists every code site loading it, intersects with the
  shop builder call graph.
- `string_xrefs.py <names…>` — finds ASCII/UTF-16 strings in the image and every `lea rip` referencing them.
- `shop_state.py` — CE-free snapshot: evaluates the per-frame release predicate for all 318 rows (expects 259 on the dev
  save), lists every live shop container/entry object. Note the 259-entry source list is a per-frame temporary and is
  only visible from inside a CE trap (`ac6.snapCount`), the scan only finds the persistent visible-list projection.
- `ce_bridge.py` — run Lua inside the running CE from the shell via `\.\pipe\CELUASERVER` + `luaclient-x86_64.dll`:
  `python ce_bridge.py 'return 42'` or `python ce_bridge.py step.lua` (dofile in CE, prints the log lines it produced).
- `ce/step14_vanilla_unlock_observer.lua` — CE trap on `SetEventFlag` (exe+B08500) logging 3xxx/6xxx flag writes with a
  backtrace; arm it (via `ce_bridge.py`) then complete a mission in the VANILLA game (DLL `enabled = false`) to find the
  mission-complete unlock handler. Output goes to `%USERPROFILE%\ac6_re_log.txt`.
- `to_mission.py [--garage] [--row N] [--kill] [--results]` — drive the game from the title screen (or the garage) into
  a mission, kill the tester AC (`kill_enemies.py --max-ap 12300`) and open the reward grid; with
  `restore_save.ps1` + `relaunch.ps1` it is the automated mission success of RE_NOTES "Test routine: instant mission
  clear".
