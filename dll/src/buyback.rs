//! Buy-back: every copy of a part sold since the last shop reroll (Vente menu or DLL auto-sale) is offered again in
//! the shop at its shop price until the next reroll. The copies are the SOLD_COPIES list in the save (up to
//! SOLD_COPY_SLOTS entries, oldest first, each lineup row + 1); a mirror in memory serves the per-frame shop predicate
//! and the ownership hook (dupshop.rs). Two copies sold = two entries = two copies offered back, one at a time.
//!
//! Sale event: the Vente sale exe+D30A80 removes the sold item through exe+D309D0(&itemId, handle, n); those calls
//! are detoured to `sale_hook`. The penalty's lost parts call RemoveItemAt directly (never through this path); DLL
//! auto-sales call `record` themselves. A sale while the list is full drops its oldest entry; the Vente confirmation
//! says so first (`confirm_text`, served by msg.rs): OUI sells, NON cancels as in vanilla. Auto-sales can't ask.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicU16;
use std::sync::{Mutex, OnceLock};

const SLOTS: usize = o::SOLD_COPY_SLOTS as usize;

/// first lineup row (for row pointer -> row index in the shop predicate)
static LINEUP_DATA: AtomicUsize = AtomicUsize::new(0);
/// inventory item id (tag | equipId) -> lineup row index
static ROW_OF: OnceLock<HashMap<u32, u16>> = OnceLock::new();
/// mirror of the SOLD_COPIES flags: lineup row + 1, oldest first, packed (0 = past the end)
static SOLD: [AtomicU16; SLOTS] = [const { AtomicU16::new(0) }; SLOTS];
/// serializes changes (game thread sales / purchases, the reroll)
static EDIT: Mutex<()> = Mutex::new(());
static BASE: AtomicUsize = AtomicUsize::new(0);

type FnShopRemove = unsafe extern "system" fn(id: *const u32, handle: u32, n: u32, flag: u32) -> i32;

pub unsafe fn init(base: usize, lu: &Lineup) {
    BASE.store(base, Relaxed);
    LINEUP_DATA.store(lu.data, Relaxed);
    let map = (0..o::ROW_COUNT).map(|i| (o::id_tag(lu.equip_type(i)) | (lu.equip_id(i) & 0x0FFF_FFFF), i as u16)).collect();
    let _ = ROW_OF.set(map);
}

/// Lineup row index of an inventory item id (tag | equipId).
pub fn row_of(item: u32) -> Option<u16> { ROW_OF.get()?.get(&item).copied() }

/// Rows of the sold copies, oldest first.
pub fn rows() -> Vec<u16> {
    SOLD.iter().map(|s| s.load(Relaxed)).take_while(|&v| v != 0).map(|v| v - 1).collect()
}
fn has(row: u16) -> bool { SOLD.iter().any(|s| s.load(Relaxed) == row + 1) }
/// No room left: the next sale drops the oldest copy.
pub fn full() -> bool { SOLD[SLOTS - 1].load(Relaxed) != 0 }

/// Mirror + flags <- `rows` (game thread or the reroll; under EDIT).
unsafe fn store(rows: &[u16]) {
    for (k, s) in SOLD.iter().enumerate() { s.store(rows.get(k).map_or(0, |r| r + 1), Relaxed); }
    let game = Game::new(BASE.load(Relaxed));
    let Some(man) = game.flag_man() else { return };
    for k in 0..SLOTS {
        game.put_bits(man, o::SOLD_COPIES + k as u32 * o::SOLD_COPY_BITS, o::SOLD_COPY_BITS, rows.get(k).map_or(0, |&r| r as u32 + 1));
    }
}

/// Shop predicate: is this lineup row (pointer) in the buy-back list?
pub unsafe fn offered(row: usize) -> bool {
    let data = LINEUP_DATA.load(Relaxed);
    if data == 0 || row < data || (row - data) % o::ROW_STRIDE != 0 { return false; }
    let i = (row - data) / o::ROW_STRIDE;
    i < o::ROW_COUNT && has(i as u16)
}

/// Shop array: a copy of `item` sold since the reroll and not bought back.
pub fn sold(item: u32) -> bool { row_of(item).is_some_and(has) }

/// Shop array: one copy bought back -> its oldest entry leaves the list.
pub unsafe fn bought_back(item: u32) {
    let Some(row) = row_of(item) else { return };
    let _g = EDIT.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = rows();
    let Some(k) = rows.iter().position(|&r| r == row) else { return };
    rows.remove(k);
    store(&rows);
    logf!("buy-back: {} {:#X} (row {}) bought back, {} cop{} left in the list", penalty::part_name(item), item, row,
        rows.len(), if rows.len() == 1 { "y" } else { "ies" });
}

/// A copy left the inventory by a sale: append it (full list: the oldest entry goes).
pub unsafe fn record(item: u32, why: &str) {
    let Some(row) = row_of(item) else { logf!("buy-back: sold {:#X} is not a lineup part ({})", item, why); return };
    let _g = EDIT.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = rows();
    if rows.len() >= SLOTS {
        let old = rows.remove(0);
        logf!("buy-back: list full, oldest copy (row {}) dropped", old);
    }
    rows.push(row);
    store(&rows);
    logf!("buy-back: {} {:#X} (row {}) sold ({}), {}/{} in the list", penalty::part_name(item), item, row, why, rows.len(), SLOTS);
}

/// Mirror <- flags (save loaded, new game).
pub unsafe fn load(game: &Game, man: usize) {
    let _g = EDIT.lock().unwrap_or_else(|e| e.into_inner());
    let rows: Vec<u16> = (0..SLOTS as u32)
        .map(|k| game.get_bits(man, o::SOLD_COPIES + k * o::SOLD_COPY_BITS, o::SOLD_COPY_BITS))
        .filter(|&v| v != 0 && v as usize <= o::ROW_COUNT).map(|v| v as u16 - 1).collect();
    for (k, s) in SOLD.iter().enumerate() { s.store(rows.get(k).map_or(0, |r| r + 1), Relaxed); }
    logf!("buy-back: {} cop{} from the save {:?}", rows.len(), if rows.len() == 1 { "y" } else { "ies" }, rows);
}

/// Shop reroll: the list starts over.
pub unsafe fn clear(_game: &Game, _man: usize) {
    let _g = EDIT.lock().unwrap_or_else(|e| e.into_inner());
    store(&[]);
    log("buy-back: cleared (reroll)");
}

/// The VENTE sale confirmation while the list is full: a line naming the copy the sale drops, then the game's own
/// text on one line (its <?itemName?> / <?price?> tags filled by the game). One text per oldest row, kept for the session.
pub fn confirm_text(category: u32, id: u32) -> Option<usize> {
    if category != o::MSG_CAT_SHOP || id != o::MSG_SELL_CONFIRM || !full() { return None; }
    let oldest = SOLD[0].load(Relaxed).checked_sub(1)?;
    static TEXTS: Mutex<Vec<(u16, usize)>> = Mutex::new(Vec::new());
    let mut texts = TEXTS.lock().ok()?;
    if let Some(t) = texts.iter().find(|t| t.0 == oldest) { return Some(t.1); }
    let vanilla = unsafe { msg::text(category, id) }?;
    let name = ROW_OF.get()?.iter().find(|e| *e.1 == oldest).map_or_else(|| format!("row {}", oldest), |e| penalty::part_name(*e.0));
    // two lines: the dialog has room for two above the credit boxes
    let text = format!("Liste de rachat pleine : {} ne sera plus rachetable.\n{}", name, vanilla.replace('\n', " "));
    let p = Box::leak(text.encode_utf16().chain(Some(0)).collect::<Vec<u16>>().into_boxed_slice()).as_ptr() as usize;
    texts.push((oldest, p));
    Some(p)
}

/// Debug: fill the free entries with `row` (full-list test).
pub unsafe fn fill(row: u16) {
    let _g = EDIT.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows = rows();
    rows.resize(SLOTS, row);
    store(&rows);
    logf!("DEBUG: buy-back list filled with row {}: {:?}", row, rows);
}

/// Detour of the Vente sale's item removal (game thread): records each copy removed.
pub unsafe extern "system" fn sale_hook(id: *const u32, handle: u32, n: u32, flag: u32) -> i32 {
    let item = *id;
    let orig: FnShopRemove = core::mem::transmute(BASE.load(Relaxed) + o::FN_SHOP_REMOVE);
    let removed = orig(id, handle, n, flag);
    for _ in 0..removed.max(0) { record(item, "Vente"); }
    if removed > 0 { event::emit(event::Event::PartSold); }
    removed
}
