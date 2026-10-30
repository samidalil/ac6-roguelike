//! Buy-back: every part sold since the last shop reroll (Vente menu or DLL auto-sale) is offered again in the shop
//! at its shop price until the next reroll. The set is the ROWS_SOLD row bitset in the save (one flag per lineup
//! row); a mirror in memory serves the per-frame shop predicate.
//!
//! Sale event: the Vente sale exe+D30A80 removes the sold item through exe+D309D0(&itemId, handle, n); those calls
//! are detoured to `sale_hook`. The penalty's lost parts call RemoveItemAt directly (never through this path); DLL
//! auto-sales call `record` themselves.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::OnceLock;

/// first lineup row (for row pointer -> row index in the shop predicate)
static LINEUP_DATA: AtomicUsize = AtomicUsize::new(0);
/// inventory item id (tag | equipId) -> lineup row index
static ROW_OF: OnceLock<HashMap<u32, u16>> = OnceLock::new();
/// mirror of the ROWS_SOLD flags
static SOLD: [AtomicU64; (o::ROW_COUNT + 63) / 64] = [const { AtomicU64::new(0) }; (o::ROW_COUNT + 63) / 64];
static BASE: AtomicUsize = AtomicUsize::new(0);

type FnShopRemove = unsafe extern "system" fn(id: *const u32, handle: u32, n: u32, flag: u32) -> i32;

pub unsafe fn init(base: usize, lu: &Lineup) {
    BASE.store(base, Relaxed);
    LINEUP_DATA.store(lu.data, Relaxed);
    let map = (0..o::ROW_COUNT).map(|i| (o::id_tag(lu.equip_type(i)) | (lu.equip_id(i) & 0x0FFF_FFFF), i as u16)).collect();
    let _ = ROW_OF.set(map);
}

fn set_bit(row: u16, v: bool) {
    let (w, b) = (row as usize / 64, row as usize % 64);
    if v { SOLD[w].fetch_or(1 << b, Relaxed); } else { SOLD[w].fetch_and(!(1 << b), Relaxed); }
}

/// Lineup row index of an inventory item id (tag | equipId).
pub fn row_of(item: u32) -> Option<u16> { ROW_OF.get()?.get(&item).copied() }

/// Shop predicate: is this lineup row (pointer) in the buy-back list?
pub unsafe fn offered(row: usize) -> bool {
    let data = LINEUP_DATA.load(Relaxed);
    if data == 0 || row < data || (row - data) % o::ROW_STRIDE != 0 { return false; }
    let i = (row - data) / o::ROW_STRIDE;
    i < o::ROW_COUNT && SOLD[i / 64].load(Relaxed) & (1 << (i % 64)) != 0
}

/// Shop array: `item` sold since the reroll and not bought back.
pub fn sold(item: u32) -> bool {
    let Some(row) = row_of(item) else { return false };
    SOLD[row as usize / 64].load(Relaxed) & (1 << (row % 64)) != 0
}

/// Shop array: bought back -> out of the list (flag + mirror).
pub unsafe fn bought_back(item: u32) {
    let Some(row) = row_of(item) else { return };
    let game = Game::new(BASE.load(Relaxed));
    if let Some(man) = game.flag_man() { game.put_row(man, o::ROWS_SOLD, row, false); }
    set_bit(row, false);
    logf!("buy-back: {} {:#X} (row {}) bought back", penalty::part_name(item), item, row);
}

/// A part left the inventory by a sale: add its row to the list (flag + mirror).
pub unsafe fn record(item: u32, why: &str) {
    let Some(&row) = ROW_OF.get().and_then(|m| m.get(&item)) else { logf!("buy-back: sold {:#X} is not a lineup part ({})", item, why); return };
    let game = Game::new(BASE.load(Relaxed));
    if let Some(man) = game.flag_man() { game.put_row(man, o::ROWS_SOLD, row, true); }
    set_bit(row, true);
    logf!("buy-back: {} {:#X} (row {}) sold ({})", penalty::part_name(item), item, row, why);
}

/// Mirror <- flags (save loaded, new game).
pub unsafe fn load(game: &Game, man: usize) {
    for w in &SOLD { w.store(0, Relaxed); }
    let rows = game.read_rows(man, o::ROWS_SOLD);
    for &r in &rows { set_bit(r, true); }
    logf!("buy-back: {} part(s) from the save {:?}", rows.len(), rows);
}

/// Shop reroll: the list starts over.
pub unsafe fn clear(game: &Game, man: usize) {
    game.clear_rows(man, o::ROWS_SOLD);
    for w in &SOLD { w.store(0, Relaxed); }
    log("buy-back: cleared (reroll)");
}

/// Detour of the Vente sale's item removal (game thread): records the item when something was removed.
pub unsafe extern "system" fn sale_hook(id: *const u32, handle: u32, n: u32, flag: u32) -> i32 {
    let item = *id;
    let orig: FnShopRemove = core::mem::transmute(BASE.load(Relaxed) + o::FN_SHOP_REMOVE);
    let removed = orig(id, handle, n, flag);
    if removed > 0 { record(item, "Vente"); event::emit(event::Event::PartSold); }
    removed
}
