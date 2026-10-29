//! Inventory cap ([inventory] max_parts): weapons, frame parts and internals owned, equipped ones included, each copy
//! one entry; core expansions do not count. Full (count >= max): the shop's buy step refuses with the game's own
//! dialog (its "none left" message reworded). Over the cap, in the garage only (a part gained there, a save loaded, back
//! from a mission): a dialog asks the player to sell, then the Vente list opens, again each time it closes while still
//! over. RE_NOTES "Inventory cap".

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32};

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
    // every part entering the inventory: the table inserts, called by AddItem (give, purchase) and by the commit
    for (site, target, f) in [(o::ADD_INSERT_HI_CALLSITE, o::FN_INV_INSERT_HI, add_insert_hi as *const () as usize),
                              (o::ADD_INSERT_LO_CALLSITE, o::FN_INV_INSERT_LO, add_insert_lo as *const () as usize),
                              (o::COMMIT_INSERT_HI_CALLSITE, o::FN_INV_INSERT_HI, commit_insert_hi as *const () as usize),
                              (o::COMMIT_INSERT_LO_CALLSITE, o::FN_INV_INSERT_LO, commit_insert_lo as *const () as usize)] {
        if let Err(e) = install_call_hook(base, site, target, f) { logf!("inventory cap: insert hook exe+{:X} FAILED: {}", site, e); }
    }
    // the VENTE factory's screen id: ours while the cap opens its screen
    match install_call_hook(base, o::SELL_SCREEN_ID_CALLSITE, o::FN_STORE_SCREEN_ID, screen_id_hook as *const () as usize) {
        Ok(cave) => logf!("inventory cap: screen id hook installed: exe+{:X} -> cave {:#X}", o::SELL_SCREEN_ID_CALLSITE, cave),
        Err(e) => logf!("inventory cap: screen id hook FAILED: {}", e),
    }
    // the list window's deleting destructor (vtable slot 1): Vente (or Achat / Assemblage) closed
    let slot = base + o::LIST_WINDOW_VTABLE + 8;
    if mem::rd_usize(slot) != base + o::FN_LIST_WINDOW_DTOR { logf!("inventory cap: list window vtable slot unexpected ({:#X})", mem::rd_usize(slot)); return; }
    let f = list_closed_hook as *const () as usize;
    if write_code(slot, &f.to_le_bytes()) { log("inventory cap: list window close hook installed"); }
}

/// The cap is opening its sell screen (set around exe+DC4B60, which runs the VENTE factory synchronously).
static OPENING: AtomicBool = AtomicBool::new(false);
/// exe+CD74F0(out, id) in the VENTE factory: stores the screen id; the cap's screen gets its own (no tutorial).
unsafe extern "system" fn screen_id_hook(out: *mut u16, id: u32) -> *mut u16 {
    *out = if OPENING.load(Relaxed) { o::CAP_SCREEN_ID } else { id as u16 };
    out
}

/// A part-list window (Vente, Achat, Assemblage) is destroyed: while over the cap, check again on the next frame.
unsafe extern "system" fn list_closed_hook(this: usize, flags: u32) -> usize {
    let orig: unsafe extern "system" fn(usize, u32) -> usize = core::mem::transmute(BASE.load(Relaxed) + o::FN_LIST_WINDOW_DTOR);
    if OVER.load(Relaxed) { CHECK.store(true, Relaxed); }
    orig(this, flags)
}

/// The inventory table inserts (exe+C2A0F0 / exe+C2A180: the two halves of the table; index or -1). Their only
/// callers: AddItem (a give or a purchase: check on the next garage frame) and the commit of the obtained items (check
/// once the garage is back: menus pushed during the results teardown are dropped).
type FnInsert = unsafe extern "system" fn(usize, usize, usize, usize) -> i32;
unsafe fn insert(target: usize, a: usize, b: usize, c: usize, d: usize, commit: bool) -> i32 {
    let f: FnInsert = core::mem::transmute(BASE.load(Relaxed) + target);
    let r = f(a, b, c, d);
    if r >= 0 { if commit { AFTER_COMMIT.store(true, Relaxed); } else { CHECK.store(true, Relaxed); } }
    r
}
unsafe extern "system" fn add_insert_hi(a: usize, b: usize, c: usize, d: usize) -> i32 { insert(o::FN_INV_INSERT_HI, a, b, c, d, false) }
unsafe extern "system" fn add_insert_lo(a: usize, b: usize, c: usize, d: usize) -> i32 { insert(o::FN_INV_INSERT_LO, a, b, c, d, false) }
unsafe extern "system" fn commit_insert_hi(a: usize, b: usize, c: usize, d: usize) -> i32 { insert(o::FN_INV_INSERT_HI, a, b, c, d, true) }
unsafe extern "system" fn commit_insert_lo(a: usize, b: usize, c: usize, d: usize) -> i32 { insert(o::FN_INV_INSERT_LO, a, b, c, d, true) }

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

// ---- the cap's sell screen: its own movie (RE_NOTES "Cap sell screen: own movie")
/// Movie names (same length: the class's string is patched in place).
const VANILLA_MOVIE: &str = "02_010_EquipTop";
const SELL_MOVIE: &str = "02_010_EquipCap";
static mut SELL_NAME_W: [u16; 16] = [0; 16];
/// Frames left before the vanilla window name is written back (0 = nothing pending).
static MOVIE_RESTORE_IN: AtomicU32 = AtomicU32::new(0);
/// Boot: make the game preload our movie, under the menu table entry of 02_OnlineArena_MatchLog (online only).
pub unsafe fn register_movie(base: usize) {
    let names = core::ptr::addr_of_mut!(SELL_NAME_W) as *mut u16;
    for (k, c) in SELL_MOVIE.encode_utf16().enumerate() { *names.add(k) = c; }
    let entry = base + o::MENU_TABLE_SELL_ENTRY;
    let old = mem::rd_usize(entry);
    core::ptr::write_unaligned(entry as *mut usize, names as usize);
    logf!("inventory cap: sell movie {} registered in menu table entry exe+{:X} (was {:#X})", SELL_MOVIE, o::MENU_TABLE_SELL_ENTRY, old);
}
/// Game thread, every frame: the vanilla window name back once our window is built.
pub unsafe fn tick_any() {
    let left = MOVIE_RESTORE_IN.load(Relaxed);
    if left == 0 { return; }
    MOVIE_RESTORE_IN.store(left - 1, Relaxed);
    if left == 1 { rewardmenu::write_wide(BASE.load(Relaxed) + o::EQUIP_TOP_MOVIE_NAME, VANILLA_MOVIE); log("inventory cap: vanilla list movie name restored"); }
}

/// Open the Vente list on the next frame.
pub static OPEN_SELL: AtomicBool = AtomicBool::new(false);
/// Our "full" dialog while it is open (its job, for exe+DC4AE0): Vente opens once it is closed. The menu manager keeps
/// one child opened on top (exe+DC4B60): opening Vente while the dialog is up would close the dialog.
static DIALOG_JOB: std::sync::Mutex<Option<(usize, usize)>> = std::sync::Mutex::new(None);

/// MSVC std::function: 0x38 bytes of small-object storage + the pointer to the callable (here the storage itself).
#[repr(C, align(16))]
struct StdFunction { storage: [usize; 7], callable: usize }

/// Open the garage's Vente list the way its menu entry does: a std::function holding the game's function-pointer
/// callable around the sell-menu factory, opened on top of the current menu like the dialog (exe+DC4B60: the job is
/// made by the menu manager and opened as its child at once).
unsafe fn open_sell(base: usize) {
    // our movie (menu/02_010_equipcap.gfx: no 3D AC view, no AC specs) for this window; vanilla name back a second later
    rewardmenu::write_wide(base + o::EQUIP_TOP_MOVIE_NAME, SELL_MOVIE);
    MOVIE_RESTORE_IN.store(60, Relaxed);
    let mut f = StdFunction { storage: [0; 7], callable: 0 };
    f.storage[0] = base + o::FN_PTR_FUNCTION_VTABLE;
    f.storage[1] = base + o::FN_SELL_MENU;
    f.callable = f.storage.as_ptr() as usize;
    type FnOpen = unsafe extern "system" fn(out: *mut [usize; 3], f: *const StdFunction) -> *mut [usize; 3];
    type FnReset = unsafe extern "system" fn(h: *mut [usize; 3]);
    let open: FnOpen = core::mem::transmute(base + o::FN_MENU_JOB_OPEN_ON_TOP);
    let reset: FnReset = core::mem::transmute(base + o::FN_MENU_JOB_RESET);
    let mut h = [0usize; 3];
    OPENING.store(true, Relaxed);
    open(&mut h, &f);
    OPENING.store(false, Relaxed);
    logf!("inventory cap: Vente opened (job {:#X})", h[1]);
    // the window is built (not opened yet): its screen id picks the menu behaviour row and so the first-time tutorial
    reset(&mut h);
}

/// Over the cap: the dialog, then the Vente list once the dialog is closed.
unsafe fn prompt(gd: usize, why: &str) {
    let (n, max) = (part_count(gd), config::get().max_parts);
    OVER.store(n > max, Relaxed);
    if n <= max { logf!("inventory cap: {} / {} parts ({})", n, max, why); return; }
    // nothing sellable (only equipped or unsellable parts): no Vente (closing it would reopen it at once); the check
    // runs again when a list window closes (Assemblage, where parts can be taken off the AC)
    let stuck = penalty::sellable_count(gd) == 0;
    logf!("inventory cap: {} / {} parts ({}) -> sell {}{}", n, max, why, n - max, if stuck { ", nothing sellable" } else { "" });
    let s = if n - max > 1 { "s" } else { "" };
    dialog::show(&if stuck { format!("INVENTAIRE PLEIN\n\n{} pièces pour {} places et aucune pièce à vendre : retirez {} pièce{} de l'AC dans ASSEMBLAGE, puis vendez-la{} dans VENTE.", n, max, n - max, s, if s.is_empty() { "" } else { "s" }) }
        else { format!("INVENTAIRE PLEIN\n\n{} pièces pour {} places : vendez {} pièce{} dans VENTE.", n, max, n - max, s) });
    let job = dialog::open_pending(BASE.load(Relaxed));
    if stuck { return; }
    match job { Some(j) => if let Ok(mut d) = DIALOG_JOB.lock() { *d = Some(j); }, None => OPEN_SELL.store(true, Relaxed) }
}

/// Game thread, garage frame: our dialog closed -> the Vente list; a part was added (or a list window closed while over
/// the cap) -> the check. After a mission commit, waits for the garage to be back (AFTER_COMMIT, cleared by lib.rs when
/// flag 3139 falls).
pub unsafe fn tick(gd: usize) {
    let base = BASE.load(Relaxed);
    if mem::rd_usize(base + o::DIALOG_REQUIRED_GLOBAL) == 0 { return; }
    // checked only while our dialog is up
    if let Ok(mut d) = DIALOG_JOB.lock() {
        if let Some((job, id)) = *d {
            let is_open: unsafe extern "system" fn(usize, usize) -> u8 = core::mem::transmute(base + o::FN_MENU_JOB_IS_OPEN);
            if is_open(job, id) == 0 { *d = None; OPEN_SELL.store(true, Relaxed); }
        }
    }
    if OPEN_SELL.swap(false, Relaxed) { open_sell(base); }
    if !AFTER_COMMIT.load(Relaxed) && CHECK.swap(false, Relaxed) { prompt(gd, "garage"); }
}
