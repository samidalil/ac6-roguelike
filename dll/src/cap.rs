//! Inventory cap ([inventory] max_parts): weapons, frame parts and internals owned, equipped ones included, each copy
//! one entry; core expansions do not count. Full (count >= max): the shop's buy step refuses with the game's own
//! dialog (its "none left" message reworded). Over the cap (a mission reward committed while full): a dialog asks the
//! player to sell, then the Vente list opens, again each time it closes while still over. RE_NOTES "Inventory cap".

use super::*;
use std::sync::atomic::AtomicBool;

static BASE: AtomicUsize = AtomicUsize::new(0);
/// Over the cap since the last check: a list window closing re-runs the check (dialog + Vente again).
static OVER: AtomicBool = AtomicBool::new(false);
/// A part may have been added (reward commit, save load): check the cap on the next garage frame.
pub static CHECK: AtomicBool = AtomicBool::new(false);
/// A mission commit added parts: check once the garage is back (lib.rs: flag 3139 falls when the post-mission sequence,
/// e.g. Walter's talk, ends). Menus pushed at the commit itself are dropped when the results screen is torn down.
pub static AFTER_COMMIT: AtomicBool = AtomicBool::new(false);

/// Inventory item ids that count: weapons (tag 0, core expansions 75xxxxxx excluded), frame parts, internals.
fn counts(item: u32) -> bool {
    match item >> 28 { 0 => (item & 0x0FFF_FFFF) / 1_000_000 != 75, 1 | 5 | 6 | 7 => true, _ => false }
}

/// Parts owned (inventory entries that count).
pub unsafe fn part_count(gd: usize) -> u32 {
    let mgr = gd + o::GD_ITEM_MGR;
    (0..penalty::capacity(mgr)).map(|i| penalty::entry(mgr, i))
        .filter(|&e| mem::rd_u32(e) != 0xFFFF_FFFF && counts(mem::rd_u32(e + 4))).count() as u32
}

pub unsafe fn install(base: usize) {
    BASE.store(base, Relaxed);
    match install_call_hook(base, o::BUY_OBTAINABLE_CALLSITE, o::FN_SHOP_OBTAINABLE_REF, buy_check_hook as *const () as usize) {
        Ok(cave) => logf!("inventory cap: buy hook installed: exe+{:X} -> cave {:#X}", o::BUY_OBTAINABLE_CALLSITE, cave),
        Err(e) => logf!("inventory cap: buy hook FAILED: {}", e),
    }
    // the list window's deleting destructor (vtable slot 1): Vente (or Achat / Assemblage) closed
    let slot = base + o::LIST_WINDOW_VTABLE + 8;
    if mem::rd_usize(slot) != base + o::FN_LIST_WINDOW_DTOR { logf!("inventory cap: list window vtable slot unexpected ({:#X})", mem::rd_usize(slot)); return; }
    let f = list_closed_hook as *const () as usize;
    if write_code(slot, &f.to_le_bytes()) { log("inventory cap: list window close hook installed"); }
}

/// A part-list window (Vente, Achat, Assemblage) is destroyed: while over the cap, check again on the next frame.
unsafe extern "system" fn list_closed_hook(this: usize, flags: u32) -> usize {
    let orig: unsafe extern "system" fn(usize, u32) -> usize = core::mem::transmute(BASE.load(Relaxed) + o::FN_LIST_WINDOW_DTOR);
    if OVER.load(Relaxed) { CHECK.store(true, Relaxed); }
    orig(this, flags)
}

/// The buy step's "can the player still get this part" call: refused (0) when the inventory is full.
unsafe extern "system" fn buy_check_hook(id: *const u32) -> i32 {
    let base = BASE.load(Relaxed);
    let orig: unsafe extern "system" fn(*const u32) -> i32 = core::mem::transmute(base + o::FN_SHOP_OBTAINABLE_REF);
    let r = orig(id);
    let Some(gd) = Game::new(base).game_data() else { return r };
    let (n, max) = (part_count(gd), config::get().max_parts);
    let refuse = r > 0 && counts(*id) && n >= max;
    if refuse { logf!("inventory cap: purchase of {:#X} refused ({} / {} parts)", *id, n, max); }
    if refuse { 0 } else { r }
}

/// msg.rs: the buy step's refusal message (cat 204, 231020, looked up before the check runs) says why while the
/// inventory is full.
pub fn refused_text(category: u32, id: u32) -> Option<usize> {
    if category != o::MSG_CAT_SHOP || id != o::MSG_SHOP_NONE_LEFT { return None; }
    let gd = unsafe { Game::new(BASE.load(Relaxed)).game_data()? };
    let max = config::get().max_parts;
    if unsafe { part_count(gd) } < max { return None; }
    static TEXT: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    Some(TEXT.get_or_init(|| format!("Inventaire plein ({} pièces maximum).\nVendez une pièce pour en acheter une autre.\0", max).encode_utf16().collect()).as_ptr() as usize)
}

/// Open the Vente list on the next garage frame (after the dialog requested with it, which then stays on top).
pub static OPEN_SELL: AtomicBool = AtomicBool::new(false);

/// MSVC std::function: 0x38 bytes of small-object storage + the pointer to the callable (here the storage itself).
#[repr(C, align(16))]
struct StdFunction { storage: [usize; 7], callable: usize }

/// Push the garage's Vente list the way its menu entry does: a menu job from a std::function holding the game's
/// function-pointer callable around the sell-menu factory.
unsafe fn open_sell(base: usize) {
    let mut f = StdFunction { storage: [0; 7], callable: 0 };
    f.storage[0] = base + o::FN_PTR_FUNCTION_VTABLE;
    f.storage[1] = base + o::FN_SELL_MENU;
    f.callable = f.storage.as_ptr() as usize;
    type FnMake = unsafe extern "system" fn(out: *mut usize, f: *const StdFunction) -> *mut usize;
    type FnPush = unsafe extern "system" fn(job: *mut usize);
    let make: FnMake = core::mem::transmute(base + o::FN_MENU_JOB_FROM_FACTORY);
    let push: FnPush = core::mem::transmute(base + o::FN_MENU_JOB_PUSH);
    let mut job = 0usize;
    make(&mut job, &f);
    if job != 0 { push(&mut job); log("inventory cap: Vente opened"); } else { log("inventory cap: Vente job not created"); }
}

/// Game thread, garage frame: over the cap after a part was added -> the sell dialog, then the Vente list.
pub unsafe fn tick(gd: usize) {
    let base = BASE.load(Relaxed);
    if mem::rd_usize(base + o::DIALOG_REQUIRED_GLOBAL) == 0 { return; }
    if OPEN_SELL.swap(false, Relaxed) { open_sell(base); }
    if !CHECK.swap(false, Relaxed) { return; }
    let (n, max) = (part_count(gd), config::get().max_parts);
    OVER.store(n > max, Relaxed);
    if n <= max { logf!("inventory cap: {} / {} parts", n, max); return; }
    logf!("inventory cap: {} / {} parts -> sell {}", n, max, n - max);
    dialog::show(&format!("INVENTAIRE PLEIN\n\n{} pièces pour {} places : vendez {} pièce{} dans VENTE.", n, max, n - max, if n - max > 1 { "s" } else { "" }));
    OPEN_SELL.store(true, Relaxed);
}
