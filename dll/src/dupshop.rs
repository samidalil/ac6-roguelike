//! The shop's offers. Weapons and frame parts can be owned several times, so the shop may roll a part the player
//! already owns. The shop asks one function how many of an item the player may still get: exe+D2F900(&id) = 1 when
//! the id is not in the inventory, else 0 (RE_NOTES "Duplicate parts in the shop"). The buy list's ownership filter,
//! the purchase confirmation, the purchasable quantity and the add itself all go through it, every list entry of every
//! frame a shop menu is up.
//!
//! The shop array answers it without reading the inventory (RE_NOTES "Shop array"): each rolled slot is for sale until
//! it is bought, each part sold since the reroll (buy-back, ROWS_SOLD) until it is bought back. Only actions change it:
//! the reroll, a purchase (the purchase's add, exe+D4E00D), a sale (buyback.rs). A copy of the same part coming
//! from anywhere else (reward, chip) leaves it alone. Starters and unique parts (booster / generator / FCS: copies would
//! share a handle) keep the game's answer (not owned).
//!
//! Save: one "bought" flag per slot (SHOP_BOUGHT), mirrored here.

use super::*;
use std::sync::atomic::AtomicBool;

/// Shop slot -> grant id (0 = empty, or a part the game decides for).
static SLOT_ID: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static SLOT_BOUGHT: [AtomicBool; 8] = [const { AtomicBool::new(false) }; 8];
static TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
static GAME_BASE: AtomicUsize = AtomicUsize::new(0);

type FnObtainable = unsafe extern "system" fn(id: *const u32) -> u32;
type FnAdd = unsafe extern "system" fn(id: *const u32, n: u32) -> u8;

/// In the array: a weapon or frame part, not a starter.
fn tracked(item: u32) -> bool {
    item != 0 && !penalty::unique_tag(item) && !penalty::FRAME_SLOTS.iter().any(|s| s.3 | s.2 == item)
}
/// Slot of `item` still for sale.
fn open_slot(item: u32) -> Option<usize> {
    (0..SLOT_ID.len()).find(|&k| SLOT_ID[k].load(Relaxed) == item && !SLOT_BOUGHT[k].load(Relaxed))
}

/// Detour of exe+D2F900 (game thread, shop menus: every list entry of every frame, the purchase steps).
unsafe extern "system" fn obtainable_hook(id: *const u32) -> u32 {
    let item = *id;
    if tracked(item) {
        if open_slot(item).is_some() || buyback::sold(item) { return 1; }
        if SLOT_ID.iter().any(|s| s.load(Relaxed) == item) { return 0; } // its slot was bought
    }
    let orig: FnObtainable = core::mem::transmute(TRAMPOLINE.load(Relaxed));
    orig(id)
}

/// The shop purchase's add (exe+D4E00D: `D2EC20(&id, n)`): the entry it sold leaves the array once the add went through
/// (D2EC20 also returns 1 when it added nothing, so what was for sale is noted before).
unsafe extern "system" fn purchase_hook(id: *const u32, n: u32) -> u8 {
    let item = *id;
    let (slot, sold) = if tracked(item) { (open_slot(item), buyback::sold(item)) } else { (None, false) };
    let add: FnAdd = core::mem::transmute(GAME_BASE.load(Relaxed) + o::FN_ADD_OBTAINABLE);
    let r = add(id, n);
    if r != 0 {
        let game = Game::new(GAME_BASE.load(Relaxed));
        if let Some(k) = slot {
            SLOT_BOUGHT[k].store(true, Relaxed);
            if let Some(man) = game.flag_man() { game.put_flag(man, o::SHOP_BOUGHT + k as u32, true); }
            logf!("shop: slot {} {} bought", k, penalty::part_name(item));
        } else if sold {
            buyback::bought_back(item);
        }
    }
    r
}

pub unsafe fn install(base: usize) {
    GAME_BASE.store(base, Relaxed);
    // mov [rsp+8],rbx; push rdi; sub rsp,20
    let prologue = [0x48, 0x89, 0x5C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x20];
    match rewardmenu::install_entry_hook(base, o::FN_SHOP_OBTAINABLE, &prologue, obtainable_hook as *const () as usize) {
        Ok(t) => { TRAMPOLINE.store(t, Relaxed); logf!("shop ownership hook installed: exe+{:X}", o::FN_SHOP_OBTAINABLE); }
        Err(e) => { logf!("shop ownership hook exe+{:X} FAILED: {} (owned parts never sold)", o::FN_SHOP_OBTAINABLE, e); return; }
    }
    match install_call_hook(base, o::PURCHASE_ADD_CALLSITE, o::FN_ADD_OBTAINABLE, purchase_hook as *const () as usize) {
        Ok(cave) => logf!("shop purchase hook installed: exe+{:X} -> cave {:#X}", o::PURCHASE_ADD_CALLSITE, cave),
        Err(e) => logf!("shop purchase hook FAILED: {} (bought slots stay for sale)", e),
    }
}

/// Mirror <- the shop slots (`parts[k]` = slot k, None = empty) and their saved "bought" flags.
pub unsafe fn load(game: &Game, man: usize, parts: &[Option<Part>]) {
    let mut bought = Vec::new();
    for k in 0..SLOT_ID.len() {
        let id = parts.get(k).copied().flatten().map(|p| p.grant_id()).filter(|&id| tracked(id)).unwrap_or(0);
        SLOT_ID[k].store(id, Relaxed);
        let b = id != 0 && game.get_flag(man, o::SHOP_BOUGHT + k as u32) == Some(true);
        SLOT_BOUGHT[k].store(b, Relaxed);
        if b { bought.push(k); }
    }
    if !bought.is_empty() { logf!("shop: bought slots {:?}", bought); }
}

/// Reroll: every slot for sale again; then the mirror.
pub unsafe fn write(game: &Game, man: usize, parts: &[Option<Part>]) {
    for k in 0..SLOT_ID.len() as u32 { game.put_flag(man, o::SHOP_BOUGHT + k, false); }
    load(game, man, parts);
}
