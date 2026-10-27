# ac6_roguelike — me3 native DLL

Roguelike mode for Armored Core VI 1.09 (Steam). Ships no `regulation.bin`: params are edited in memory, game code
is hooked with small call-site caves. All addresses live in `src/offsets.rs` and are documented in `../RE_NOTES.md`.

## Build
Needs the Rust MSVC toolchain (`x86_64-pc-windows-msvc`). Close the game first: it locks the DLL.
```
cargo build --release
```
Output: `target/release/ac6_roguelike.dll`, loaded by the `[[natives]]` entry of `../ac6-roguelike-fresh3.me3`.
Release package (DLL + profile + README + zip in `../dist`): `powershell -File ../tools/package.ps1`.

## Run
```
me3 launch --auto-detect -p ../ac6-roguelike-fresh3.me3
```
Log: `%USERPROFILE%\ac6_roguelike_dll.log` (rewritten at every launch).
Settings: `ac6_roguelike.toml` next to the DLL, written from `src/default_config.toml` when missing.

## Modules
- `lib.rs` — boot (config, param lookup, hooks), the shop (pool, rarity pick, reroll at mission end, state kept in
  event flags), mission rewards (queued at mission start through the game's give path), the game-thread loop and the
  game-over exit sequence.
- `penalty.rs` — failure plan built at sortie (escrow flag), penalty on the next garage load, auto-sell, game over,
  part values (shop price, stat model, `[prices]` overrides) and the sell-price hook.
- `config.rs` / `default_config.toml` — settings file parser and defaults (economy, rewards, shop, rarity, prices).
- `dialog.rs` — the game's OK dialog, opened with our own text; `msg.rs` — message lookup hook (custom texts).
- `offsets.rs`, `mem.rs` — addresses, memory helpers (logging is in `lib.rs`).

## Status
Playable end to end: mission pay removed, random shop and rewards by rarity tier, failed sorties take parts, auto-sell
when broke, game over back to the title with the run's save closed. Vanilla rank / arena reward parts are not granted;
they are shop parts like the others.
