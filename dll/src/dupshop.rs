//! Duplicate parts in the shop. Weapons and frame parts can be owned several times, so the shop may roll a part the
//! player already owns. The shop asks one function how many of an item the player may still get: exe+D2F900(&id) =
//! 1 when the id is not in the inventory, else 0 (RE_NOTES "Duplicate parts in the shop"). The buy list's ownership
//! filter, the purchase confirmation, the purchasable quantity and the add itself all go through it, so an owned part
//! is hidden, refused ("Vous avez acheté toutes les pièces disponibles.") or charged without being added. Hooked at
//! its entry: a shop slot's part answers 1 while the player owns no more copies of it than when the slot was rolled
//! (its baseline), so it is listed and bought like any part, and once bought it is hidden until the next reroll.
//! Every other item gets the game's answer (buy-back, starters, booster / generator / FCS).
//!
//! Baselines: 3 bits per shop slot in the save (flags SHOP_BASELINE..), written at the reroll, mirrored here.

use super::*;

const BITS: u32 = 3;
/// Shop slot -> (grant id, copies owned when rolled); grant id 0 = the game decides.
static SLOT_ID: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static SLOT_BASE: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
static GAME_BASE: AtomicUsize = AtomicUsize::new(0);

type FnObtainable = unsafe extern "system" fn(id: *const u32) -> u32;

/// Detour of exe+D2F900 (game thread, shop menus: every list entry of every frame the buy list is built, the
/// purchase steps).
unsafe extern "system" fn obtainable_hook(id: *const u32) -> u32 {
    let item = *id;
    if item != 0 {
        if let Some(k) = SLOT_ID.iter().position(|s| s.load(Relaxed) == item) {
            let game = Game::new(GAME_BASE.load(Relaxed));
            if let Some(gd) = game.game_data() {
                let copies = game.owned_ids(gd).iter().filter(|x| **x == item).count() as u32;
                return (copies <= SLOT_BASE[k].load(Relaxed)) as u32;
            }
        }
    }
    let orig: FnObtainable = core::mem::transmute(TRAMPOLINE.load(Relaxed));
    orig(id)
}

pub unsafe fn install(base: usize) {
    GAME_BASE.store(base, Relaxed);
    // mov [rsp+8],rbx; push rdi; sub rsp,20
    let prologue = [0x48, 0x89, 0x5C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x20];
    match rewardmenu::install_entry_hook(base, o::FN_SHOP_OBTAINABLE, &prologue, obtainable_hook as *const () as usize) {
        Ok(t) => { TRAMPOLINE.store(t, Relaxed); logf!("shop ownership hook installed: exe+{:X}", o::FN_SHOP_OBTAINABLE); }
        Err(e) => logf!("shop ownership hook exe+{:X} FAILED: {} (owned parts never sold)", o::FN_SHOP_OBTAINABLE, e),
    }
}

/// Mirror <- the shop slots (`parts[k]` = slot k, None = empty) and their saved baselines. Only weapons and frame
/// parts get the bypass.
pub unsafe fn load(game: &Game, man: usize, parts: &[Option<Part>]) {
    for k in 0..SLOT_ID.len() {
        let p = parts.get(k).copied().flatten().filter(|p| !p.unique());
        SLOT_ID[k].store(p.map(|p| p.grant_id()).unwrap_or(0), Relaxed);
        SLOT_BASE[k].store(game.get_bits(man, o::SHOP_BASELINE + BITS * k as u32, BITS), Relaxed);
    }
}

/// Reroll: each slot's baseline = copies of its part owned now (7 at most), saved; then the mirror.
pub unsafe fn write(game: &Game, gd: usize, man: usize, parts: &[Option<Part>]) {
    let owned = game.owned_ids(gd);
    let mut v = Vec::new();
    for k in 0..SLOT_ID.len() {
        let n = parts.get(k).copied().flatten().map(|p| owned.iter().filter(|x| **x == p.grant_id()).count()).unwrap_or(0);
        game.put_bits(man, o::SHOP_BASELINE + BITS * k as u32, BITS, n.min(7) as u32);
        if k < parts.len() { v.push(n); }
    }
    logf!("shop baselines (copies owned per slot) {:?}", v);
    load(game, man, parts);
}

/// Saves from before duplicates kept the "already rolled" bitsets where the baselines now live: clear those ranges
/// once, then mark the save (8-bit format value). The shops of those saves only hold unowned parts: baseline 0.
pub unsafe fn migrate(game: &Game, man: usize) {
    if game.get_bits(man, o::SAVE_FORMAT, 8) == o::SAVE_FORMAT_DUPES { return; }
    for i in 0..o::ROW_COUNT as u32 { game.put_flag(man, o::OLD_ROWS_ROLLED + i, false); }
    for i in 0..o::OLD_REWARD_ROLLED_A_LEN { game.put_flag(man, o::OLD_REWARD_ROLLED_A + i, false); }
    for i in 0..(o::ROW_COUNT as u32 - o::OLD_REWARD_ROLLED_A_LEN) { game.put_flag(man, o::OLD_REWARD_ROLLED_B + i, false); }
    game.put_bits(man, o::SAVE_FORMAT, 8, o::SAVE_FORMAT_DUPES);
    log("save format: old rolled-part sets cleared (duplicates allowed)");
}
