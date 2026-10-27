//! Failure penalty + negative-balance auto-sell, executed on the GAME THREAD.
//!
//! Game code (remove item, give item) must not be called from the DLL's own thread (that froze the
//! game once). We detour the per-frame save step call at exe+1274418 to `frame_hook`, which runs the
//! original step and then any work the DLL thread queued through the atomics below.
//!
//! Rules (RE_NOTES "Penalty rules decided 2026-10-26"):
//! * every equipped weapon is lost, its slot left empty;
//! * every equipped frame part / internal that is not the starter is lost and replaced by the starter
//!   (re-granted if missing); a slot already holding its starter costs the starter's price instead;
//! * a negative balance sells stash parts (not equipped), most expensive first; still negative with
//!   nothing left = game over.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::{Mutex, OnceLock};

/// The DLL thread sets these; the game thread consumes them in `frame_hook`.
pub static PENALTY_REQUEST: AtomicBool = AtomicBool::new(false);
pub static CLEANUP_REQUEST: AtomicBool = AtomicBool::new(false);
/// A deficit to cover by auto-selling (unclamped negative balance seen by the COAM detour), 0 = none.
pub static AUTOSELL_DEFICIT: AtomicI32 = AtomicI32::new(0);
/// Set when the death menu opens: the game thread retires the mission on the next frame (rule: death goes
/// straight back to the garage, no Retry).
pub static RETIRE_REQUEST: AtomicBool = AtomicBool::new(false);
pub static RETIRED: AtomicUsize = AtomicUsize::new(0);
pub static GAME_OVER_PENDING: AtomicBool = AtomicBool::new(false);


static BASE: AtomicUsize = AtomicUsize::new(0);
static CTX: OnceLock<Ctx> = OnceLock::new();

pub struct Ctx {
    /// sell value by inventory item id (tag | equipId): part_/weapon_sell_percent of the part's value
    pub price_of: HashMap<u32, i32>,
    /// starter price per frame/internal slot (index into FRAME_SLOTS)
    pub starter_price: [i32; 7],
}

/// (offset in PlayerGameData, slot name, starter equipId, inventory id tag)
pub const FRAME_SLOTS: [(usize, &str, u32, u32); 7] = [
    (0x53C, "head", 50070400, 0x1000_0000),
    (0x540, "core", 51070400, 0x1000_0000),
    (0x544, "arms", 52070400, 0x1000_0000),
    (0x548, "legs", 53070400, 0x1000_0000),
    (0x550, "booster", 60070400, 0x6000_0000),
    (0x554, "generator", 65000000, 0x5000_0000),
    (0x558, "fcs", 70000000, 0x7000_0000),
];
/// left arm, right arm, left back, right back
pub const WEAPON_SLOTS: [(usize, &str); 4] = [(0x4F4, "L-arm"), (0x4F8, "R-arm"), (0x4FC, "L-back"), (0x500, "R-back")];
const EMPTY: u32 = 0xFFFF_FFFF;
/// The assembly keeps THREE parallel per-slot arrays in PlayerGameData (slot k): inventory INDEX at
/// +0x24C+4k, inventory HANDLE at +0x424+4k, part ID at +0x4F4+4k. The Assembly menu resolves a slot
/// through the index; all three must agree (verified 2026-10-26: a stale index shows "(NEANT)").
const HANDLE_DELTA: usize = 0xD0;
const INDEX_DELTA: usize = 0x2A8;

type FnSaveStep = unsafe extern "system" fn(this: usize);
type FnRemoveItemAt = unsafe extern "system" fn(mgr: usize, index: u32, flag: u32) -> u64;

/// Price class of a lineup row: FRAME_SLOTS index for frame parts / internals, WEAPON_CLASS for weapons. Core
/// expansions (75xxxxxx) have none: not sellable.
const WEAPON_CLASS: usize = 7;
fn price_class(kind: u8, id: u32) -> Option<usize> {
    match (kind, id / 1_000_000) {
        (0, 75) => None,
        (0, _) => Some(WEAPON_CLASS),
        (1, 50) => Some(0), (1, 51) => Some(1), (1, 52) => Some(2), (1, 53) => Some(3),
        (4, _) => Some(4), (6, _) => Some(5), (5, _) => Some(6), _ => None,
    }
}

/// Build the price tables from the lineup (DLL thread, at boot). A part's value is the game's shop price; starter
/// parts (price 0) are worth the cheapest shop part of their class; a [prices] entry replaces the value. Parts sell
/// for `part_sell_percent` / `weapon_sell_percent` of their value and cost `item_charge_percent` of it: the shop
/// price is rewritten to that cost, and a failure charges it for each slot already on its starter.
pub unsafe fn init(base: usize, lu: &Lineup) {
    BASE.store(base, Relaxed);
    msg::set_base(base);
    let cfg = config::get();
    let pct = |v: i32, p: u32| (v as i64 * p as i64 / 100) as i32;
    // reference value per class: the cheapest shop part
    let mut reference = [i32::MAX; 8];
    for i in 0..o::ROW_COUNT {
        let (id, price, kind) = (lu.equip_id(i), lu.price(i), lu.equip_type(i));
        if price <= 0 || lu.reward_cond(i) != 0 { continue; }
        if let Some(c) = price_class(kind, id) { reference[c] = reference[c].min(price); }
    }
    for p in reference.iter_mut() { if *p == i32::MAX { *p = 0; } }
    // overrides by equipId or by name key
    let mut by_id: HashMap<u32, u32> = HashMap::new();
    let mut by_name: HashMap<String, (String, u32)> = HashMap::new();
    for (k, v) in &cfg.prices {
        match k.parse::<u32>() { Ok(id) => { by_id.insert(id, *v); } Err(_) => { by_name.insert(config::name_key(k), (k.clone(), *v)); } }
    }
    let mut used: Vec<String> = Vec::new();
    let mut price_of = HashMap::new();
    let mut value_of: HashMap<u32, i32> = HashMap::new();
    for i in 0..o::ROW_COUNT {
        let (id, price, kind) = (lu.equip_id(i), lu.price(i), lu.equip_type(i));
        if lu.reward_cond(i) != 0 { continue; }
        let Some(c) = price_class(kind, id) else { continue };
        let item = o::id_tag(kind) | (id & 0x0FFF_FFFF);
        let name = part_name(item);
        let over = by_id.get(&id).copied().or_else(|| by_name.get(&config::name_key(&name)).map(|(k, v)| { used.push(k.clone()); *v }));
        let value = match over {
            Some(v) => { let v = v.min(i32::MAX as u32) as i32; logf!("price override: {} ({}) {} -> {}", name, id, price, v); v }
            None if price > 0 => price,
            None => reference[c],
        };
        let (sell_pct, charge_pct) = (if c == WEAPON_CLASS { cfg.weapon_sell_percent } else { cfg.part_sell_percent }, cfg.item_charge_percent);
        // shop buy price = the charge (only rows the shop can sell: priced by the game, or given a value)
        if price > 0 || over.is_some() { mem::wr_i32(lu.row(i) + o::ROW_PRICE, pct(value, charge_pct)); }
        value_of.insert(item, value);
        let sell = pct(value, sell_pct);
        if sell > 0 { price_of.insert(item, sell); }
    }
    for (key, (k, _)) in &by_name {
        if !used.contains(k) { logf!("price override \"{}\": no part with that name (key {})", k, key); }
    }
    let mut starter_price = [0i32; 7];
    for (k, (_, name, starter, tag)) in FRAME_SLOTS.iter().enumerate() {
        let value = value_of.get(&(tag | starter)).copied().unwrap_or(reference[k]);
        starter_price[k] = pct(value, cfg.item_charge_percent);
        logf!("starter {} {}: value {}, failure charge {}, sells for {}", name, starter, value, starter_price[k], pct(value, cfg.part_sell_percent));
    }
    let _ = CTX.set(Ctx { price_of, starter_price });
}

/// Sell value of an inventory item id (tag | equipId) from our price table (value x sell %), None if not priced.
pub fn sell_value(item: u32) -> Option<i32> { CTX.get()?.price_of.get(&item).copied() }

static SELL_HOOK_LOGS: AtomicUsize = AtomicUsize::new(0);

/// Replaces the calls to the game's sale value getter (Vente menu, sale): our value x sell % for priced parts.
pub unsafe extern "system" fn sell_price_hook(desc: usize) -> i32 {
    let base = BASE.load(Relaxed);
    let orig: unsafe extern "system" fn(usize) -> i32 = core::mem::transmute(base + o::FN_SELL_PRICE);
    let game_value = orig(desc);
    let item = mem::rd_u32(desc + o::SELL_DESC_ITEM_ID);
    let v = sell_value(item).unwrap_or(game_value);
    if SELL_HOOK_LOGS.fetch_add(1, Relaxed) < 8 { logf!("sell price: item {:#X} game {} -> {}", item, game_value, v); }
    v
}

/// Detour the per-frame save step call (exe+1274418: call exe+1272440) to `frame_hook`.
pub unsafe fn install(base: usize) -> Result<usize, String> {
    BASE.store(base, Relaxed);
    install_call_hook(base, o::FRAME_HOOK_CALLSITE, o::FN_SAVE_STEP, frame_hook as *const () as usize)
}

/// Runs on the game thread every frame, right after the game's own save step.
unsafe extern "system" fn frame_hook(this: usize) {
    let base = BASE.load(Relaxed);
    let orig: FnSaveStep = core::mem::transmute(base + o::FN_SAVE_STEP);
    orig(this);
    let Some(ctx) = CTX.get() else { return };
    let game = Game::new(base);
    let Some(gd) = game.game_data() else { return };
    if RETIRE_REQUEST.swap(false, Relaxed) && game.defer_byte(gd) == 1 {
        let retire: unsafe extern "system" fn() -> u8 = core::mem::transmute(base + o::FN_RETIRE_MISSION);
        retire();
        RETIRED.fetch_add(1, Relaxed);
        return;
    }
    if game.defer_byte(gd) != 0 { return; } // the rest is garage-only work
    dialog::open_pending(base);
    if CLEANUP_REQUEST.swap(false, Relaxed) { cleanup_orphans(&game, gd); }
    if PENALTY_REQUEST.swap(false, Relaxed) { apply_penalty(&game, ctx, gd); }
    let deficit = AUTOSELL_DEFICIT.swap(0, Relaxed);
    if deficit < 0 { auto_sell(&game, ctx, gd, deficit); }
}

// ---------------------------------------------------------------- inventory helpers
/// Address of inventory entry `i` ({handle, id, count, ordinal}); the first [mgr+1C] entries live at
/// [mgr+40], the rest at [mgr+30].
unsafe fn entry(mgr: usize, i: u32) -> usize {
    let split = mem::rd_u32(mgr + o::MGR_TABLE_SPLIT);
    if i < split { mem::rd_usize(mgr + o::MGR_TABLE_LO) + i as usize * 16 }
    else { mem::rd_usize(mgr + o::MGR_TABLE_HI) + (i - split) as usize * 16 }
}
unsafe fn capacity(mgr: usize) -> u32 { mem::rd_u32(mgr + o::MGR_TABLE_COUNT).min(4096) }

/// Inventory index holding item `id` (tag | equipId).
unsafe fn find(mgr: usize, id: u32) -> Option<u32> {
    (0..capacity(mgr)).find(|&i| { let e = entry(mgr, i); mem::rd_u32(e) != EMPTY && mem::rd_u32(e + 4) == id })
}

unsafe fn remove_at(game: &Game, gd: usize, index: u32) {
    let f: FnRemoveItemAt = core::mem::transmute(game.base + o::FN_REMOVE_ITEM_AT);
    f(gd + o::GD_ITEM_MGR, index, 1);
}

/// Remove one owned item by id. Returns false if it was not in the inventory.
unsafe fn remove_item(game: &Game, gd: usize, id: u32) -> bool {
    match find(gd + o::GD_ITEM_MGR, id) { Some(i) => { remove_at(game, gd, i); true } None => false }
}

/// Make sure the player owns `id`; grants it directly (garage: no deferral) if missing.
unsafe fn ensure_owned(game: &Game, gd: usize, id: u32) -> bool {
    if find(gd + o::GD_ITEM_MGR, id).is_some() { return false; }
    let mut buf = [0u8; 0x40];
    let d = game.desc(id, &mut buf);
    (game.give_item)(gd + o::GD_GIVE_OBJ, d, 1, 1, 1, 1, 1);
    true
}

unsafe fn find_handle(mgr: usize, handle: u32) -> Option<u32> {
    (0..capacity(mgr)).find(|&i| mem::rd_u32(entry(mgr, i)) == handle)
}

/// Remove one owned item by inventory handle (falls back to the id if the handle is gone).
unsafe fn remove_handle(game: &Game, gd: usize, handle: u32, id: u32) -> bool {
    match find_handle(gd + o::GD_ITEM_MGR, handle) { Some(i) => { remove_at(game, gd, i); true } None => remove_item(game, gd, id) }
}

/// Inventory handles of everything equipped (weapons + frame/internals).
unsafe fn equipped_handles(gd: usize) -> Vec<u32> {
    WEAPON_SLOTS.iter().map(|(off, _)| *off).chain(FRAME_SLOTS.iter().map(|(off, ..)| *off))
        .map(|off| mem::rd_u32(gd + off - HANDLE_DELTA)).filter(|&h| h != EMPTY).collect()
}

/// Sellable stash: owned, not equipped, priced parts, most expensive first: (price, handle, id).
/// Handles (not ids) so a second copy of an equipped part counts as stash.
unsafe fn stash(ctx: &Ctx, gd: usize) -> Vec<(i32, u32, u32)> {
    let mgr = gd + o::GD_ITEM_MGR;
    let equipped = equipped_handles(gd);
    let mut v: Vec<(i32, u32, u32)> = Vec::new();
    for i in 0..capacity(mgr) {
        let e = entry(mgr, i);
        let (h, id) = (mem::rd_u32(e), mem::rd_u32(e + 4));
        if h == EMPTY || equipped.contains(&h) { continue; }
        if let Some(&p) = ctx.price_of.get(&id) { if p > 0 { v.push((p, h, id)); } }
    }
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v
}

// ---------------------------------------------------------------- failure plan
/// Everything a failed sortie costs, computed once when the sortie is committed (deployment talk). The loadout
/// and inventory cannot change during a mission, and the escrow save is written at that same moment, so a plan
/// rebuilt from that save on load is identical. The death check and the garage penalty both use this plan.
pub struct FailurePlan {
    /// weapon slots emptied: (id offset in PlayerGameData, slot name, handle, id)
    weapons: Vec<(usize, &'static str, u32, u32)>,
    /// frame slots swapped to their starter: (FRAME_SLOTS index, lost handle, lost id, spare starter handle if owned)
    swaps: Vec<(usize, u32, u32, Option<u32>)>,
    /// frame slots already on their starter (FRAME_SLOTS indices): each costs the starter's price
    keep: Vec<usize>,
    starter_cost: i64,
    /// stash parts sold to cover the cost, most expensive first (spare starters about to be equipped excluded)
    sell: Vec<(i32, u32, u32)>,
}

impl FailurePlan {
    unsafe fn build(ctx: &Ctx, gd: usize) -> FailurePlan {
        let mgr = gd + o::GD_ITEM_MGR;
        let mut sell = stash(ctx, gd);
        let weapons = WEAPON_SLOTS.iter().filter_map(|&(off, name)| {
            let id = mem::rd_u32(gd + off);
            if id == EMPTY { None } else { Some((off, name, mem::rd_u32(gd + off - HANDLE_DELTA), id)) }
        }).collect();
        let (mut swaps, mut keep, mut starter_cost) = (Vec::new(), Vec::new(), 0i64);
        for (k, (off, _, starter, tag)) in FRAME_SLOTS.iter().enumerate() {
            let id = mem::rd_u32(gd + off);
            if id == *starter { keep.push(k); starter_cost += ctx.starter_price[k] as i64; continue; }
            // a spare starter in the stash gets equipped: it is no longer for sale
            let spare = (0..capacity(mgr)).map(|i| entry(mgr, i))
                .find(|&e| mem::rd_u32(e + 4) == (tag | starter) && sell.iter().any(|s| s.1 == mem::rd_u32(e)))
                .map(|e| mem::rd_u32(e));
            if let Some(h) = spare { sell.retain(|s| s.1 != h); }
            swaps.push((k, mem::rd_u32(gd + off - HANDLE_DELTA), id, spare));
        }
        FailurePlan { weapons, swaps, keep, starter_cost, sell }
    }

    /// Balance left after the penalty and selling everything sellable; negative = game over.
    pub fn balance(&self, coam: i64) -> i64 {
        coam - self.starter_cost + self.sell.iter().map(|s| s.0 as i64).sum::<i64>()
    }

    fn summary(&self) -> String {
        format!("{} weapon(s) lost, {} part(s) swapped to starters, {} starter(s) to pay ({}), {} sellable part(s) worth {}",
            self.weapons.len(), self.swaps.len(), self.keep.len(), self.starter_cost,
            self.sell.len(), self.sell.iter().map(|s| s.0 as i64).sum::<i64>())
    }
}

static PLAN: Mutex<Option<FailurePlan>> = Mutex::new(None);

/// Compute and keep the failure plan of the sortie being committed (DLL thread, memory reads only).
/// Returns the balance the player would have after a failure.
pub unsafe fn arm(gd: usize, why: &str) -> Option<i64> {
    let ctx = CTX.get()?;
    let plan = FailurePlan::build(ctx, gd);
    let bal = plan.balance(mem::rd_i32(gd + o::GD_COAM) as i64);
    logf!("failure plan ({}): {} -> balance after a failure {}", why, plan.summary(), bal);
    *PLAN.lock().unwrap() = Some(plan);
    Some(bal)
}

pub fn disarm() { *PLAN.lock().unwrap() = None; }

/// Balance after a failure according to the armed plan, at the current COAM.
pub unsafe fn failure_balance(gd: usize) -> Option<i64> {
    PLAN.lock().unwrap().as_ref().map(|p| p.balance(mem::rd_i32(gd + o::GD_COAM) as i64))
}

// ---------------------------------------------------------------- penalty
unsafe fn apply_penalty(game: &Game, ctx: &Ctx, gd: usize) {
    let plan = PLAN.lock().unwrap().take();
    let plan = plan.unwrap_or_else(|| { log("penalty: no armed plan, building one now"); FailurePlan::build(ctx, gd) });
    let mgr = gd + o::GD_ITEM_MGR;
    let mut lines: Vec<String> = Vec::new();
    let mut sold: Vec<(u32, i32)> = Vec::new();
    for &(off, name, handle, id) in &plan.weapons {
        mem::wr_u32(gd + off - INDEX_DELTA, EMPTY); // unequip first (index + handle + id), then remove
        mem::wr_u32(gd + off - HANDLE_DELTA, EMPTY);
        mem::wr_u32(gd + off, EMPTY);
        let removed = remove_handle(game, gd, handle, id);
        lines.push(format!("lost {} weapon {}{}", name, id, if removed { "" } else { " (was not in the inventory)" }));
    }
    for &k in &plan.keep {
        lines.push(format!("{} is the starter: pay {}", FRAME_SLOTS[k].1, ctx.starter_price[k]));
    }
    for &(k, handle, id, spare) in &plan.swaps {
        let (off, name, starter, tag) = FRAME_SLOTS[k];
        let granted = spare.is_none() && ensure_owned(game, gd, tag | starter);
        // equip the starter first (index + handle + id), then remove the lost part
        let si = spare.and_then(|h| find_handle(mgr, h)).or_else(|| find(mgr, tag | starter));
        let Some(si) = si else { lines.push(format!("{}: starter {} missing after grant!", name, starter)); continue };
        mem::wr_u32(gd + off - INDEX_DELTA, si);
        mem::wr_u32(gd + off - HANDLE_DELTA, mem::rd_u32(entry(mgr, si)));
        mem::wr_u32(gd + off, starter);
        if id != EMPTY { remove_handle(game, gd, handle, tag | id); }
        lines.push(format!("lost {} {} -> starter {}{}", name, id, starter, if granted { " (re-granted)" } else { "" }));
    }
    let bal = mem::rd_i32(gd + o::GD_COAM) as i64 - plan.starter_cost;
    if plan.starter_cost > 0 { lines.push(format!("starter costs: {} (balance {} -> {})", plan.starter_cost, bal + plan.starter_cost, bal)); }
    let over = if bal < 0 {
        sell(game, gd, &plan.sell, bal, &mut lines, &mut sold)
    } else {
        mem::wr_i32(gd + o::GD_COAM, bal as i32);
        false
    };
    // consumed: clear the escrow flag and save
    if let Some(man) = game.flag_man() { game.put_flag(man, o::FLAG_ESCROW, false); }
    game.request_save();
    log("PENALTY APPLIED:");
    for l in &lines { logf!("  {}", l); }
    if !over { dialog::show(&loss_report(&plan, &sold)); }
}

/// Text of the loss dialog shown in the garage after a failed sortie (game language: French). The game's dialog
/// box does not grow past ~12 lines (verified), so one line per category; long lines wrap inside the box.
fn loss_report(plan: &FailurePlan, sold: &[(u32, i32)]) -> String {
    let mut t = String::from("ÉCHEC DE LA MISSION : PERTES\n\n");
    if !plan.weapons.is_empty() {
        let names: Vec<String> = plan.weapons.iter().map(|w| part_name(w.3)).collect();
        t += &format!("Armes perdues : {}\n", names.join(", "));
    }
    if !plan.swaps.is_empty() {
        let names: Vec<String> = plan.swaps.iter().map(|s| part_name(FRAME_SLOTS[s.0].3 | s.2)).collect();
        t += &format!("Remplacées par les pièces de départ : {}\n", names.join(", "));
    }
    if plan.starter_cost > 0 {
        t += &format!("Pièces de départ facturées : {} COAM\n", money(plan.starter_cost));
    }
    if !sold.is_empty() {
        let names: Vec<String> = sold.iter().map(|&(item, p)| format!("{} ({})", part_name(item), money(p as i64))).collect();
        t += &format!("Vendues : {}\n", names.join(", "));
    }
    t.trim_end().to_string()
}

/// Display name of an inventory item id (tag | equipId), from the game's own part-name texts.
pub fn part_name(item: u32) -> String {
    let id = item & 0x0FFF_FFFF;
    let cat = match item >> 28 {
        0 => o::MSG_CAT_WEAPON_NAME, 1 => o::MSG_CAT_FRAME_NAME, 6 => o::MSG_CAT_BOOSTER_NAME,
        5 => o::MSG_CAT_GENERATOR_NAME, 7 => o::MSG_CAT_FCS_NAME, _ => return format!("#{}", id),
    };
    unsafe { msg::text(cat, id) }.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("#{}", id))
}

/// 1234567 -> "1 234 567" (the game's own thousands separator).
fn money(v: i64) -> String {
    let d = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 { out.push(' '); }
        out.push(c);
    }
    if v < 0 { format!("-{}", out) } else { out }
}

/// Cover `deficit` (< 0) by selling `parts` in order and set the balance. Still short with nothing left = game over
/// (returns true).
unsafe fn sell(game: &Game, gd: usize, parts: &[(i32, u32, u32)], deficit: i64, lines: &mut Vec<String>, sold: &mut Vec<(u32, i32)>) -> bool {
    let mut bal = deficit;
    for &(p, h, id) in parts {
        if bal >= 0 { break; }
        if remove_handle(game, gd, h, id) {
            bal += p as i64;
            sold.push((id, p));
            lines.push(format!("auto-sold {:#X} for {}", id, p));
        }
    }
    mem::wr_i32(gd + o::GD_COAM, bal.max(0) as i32);
    if bal < 0 {
        lines.push(format!("GAME OVER: still {} short with nothing left to sell -> run over, back to the title", -bal));
        if let Some(man) = game.flag_man() { game.put_flag(man, o::FLAG_RUN_OVER, true); }
        game.request_save();
        GAME_OVER_PENDING.store(true, Relaxed); // the DLL thread sends us to the title once the save is written
        return true;
    }
    false
}

/// A negative balance in the garage (seen by the COAM detour): sell stash parts, most expensive first.
unsafe fn auto_sell(game: &Game, ctx: &Ctx, gd: usize, deficit: i32) {
    let (mut lines, mut sold) = (Vec::new(), Vec::new());
    let over = sell(game, gd, &stash(ctx, gd), deficit as i64, &mut lines, &mut sold);
    log("AUTO-SELL:");
    for l in &lines { logf!("  {}", l); }
    if !over && !sold.is_empty() {
        let names: Vec<String> = sold.iter().map(|&(item, p)| format!("{} ({})", part_name(item), money(p as i64))).collect();
        let t = format!("SOLDE NÉGATIF : VENTE AUTOMATIQUE\n\nVendues : {}", names.join(", "));
        dialog::show(&t);
    }
}

/// Remove inventory entries whose id carries the frame tag on a non-frame part (left by an early build
/// that granted boosters with the wrong tag: they show nameless and cannot be used).
unsafe fn cleanup_orphans(game: &Game, gd: usize) {
    let mgr = gd + o::GD_ITEM_MGR;
    let mut fixed = 0;
    for i in 0..capacity(mgr) {
        let e = entry(mgr, i);
        let (h, id) = (mem::rd_u32(e), mem::rd_u32(e + 4));
        if h == EMPTY || id >> 28 != 1 { continue; }
        let prefix = (id & 0x0FFF_FFFF) / 1_000_000;
        if !(50..=53).contains(&prefix) { remove_at(game, gd, i); fixed += 1; logf!("cleanup: removed mis-tagged item {:#X} (entry {})", id, i); }
    }
    if fixed > 0 { game.request_save(); }
}

type FnOpenRetryMenu = unsafe extern "system" fn(out: usize, mode: u32, a3: u32, a4: u32) -> usize;

/// Replaces the event script's call that opens the death menu: build it as usual (the caller uses the
/// result), then ask the game thread to retire the mission on the next frame.
pub unsafe extern "system" fn death_menu_hook(out: usize, mode: u32, a3: u32, a4: u32) -> usize {
    let base = BASE.load(Relaxed);
    if mode != 1 {
        let orig: FnOpenRetryMenu = core::mem::transmute(base + o::FN_OPEN_RETRY_MENU);
        return orig(out, mode, a3, a4);
    }
    // The death menu is never built (building it showed it for a frame): hand the script an empty menu-job handle
    // (the caller only resets it) and retire to the garage on the next frame. A death that ends the run leaves for
    // the title from there: saves are impossible in a mission, and the session-end save must be written.
    let h = out as *mut usize;
    *h = base + o::MENU_JOB_HANDLE_VTABLE;
    *h.add(1) = 0;
    *h.add(2) = 0;
    RETIRE_REQUEST.store(true, Relaxed);
    out
}



/// Return to the title screen the way the system menu does: raise the menu manager's "leave map" request.
/// With no destination area set (-1, the garage's normal state) the game exits the map and shows the title.
/// A plain byte write: safe from any thread.
pub unsafe fn request_title(base: usize) -> bool {
    let mm = mem::rd_usize(base + o::MENU_MAN_GLOBAL);
    if mm == 0 || !mem::is_readable(mm + o::MENU_MAN_SUB, 8) { return false; }
    let sub = mem::rd_usize(mm + o::MENU_MAN_SUB);
    if sub == 0 || !mem::is_readable(sub + o::MENU_LEAVE_MAP_REQUEST, 1) { return false; }
    let st = mem::rd_usize(base + o::SAVE_STATE_GLOBAL);
    if st != 0 && mem::is_readable(st + o::SAVE_DESTINATION, 4) {
        let dest = mem::rd_i32(st + o::SAVE_DESTINATION);
        if dest != -1 { logf!("request_title: destination was {}, forcing -1 (title)", dest); mem::wr_i32(st + o::SAVE_DESTINATION, -1); }
    }
    clear_session_flag(base);
    mem::wr_u8(sub + o::MENU_LEAVE_MAP_REQUEST, 1);
    true
}

/// Set while a game-over exit is in progress: the game must not re-open the session (it does when a mission returns
/// to the garage, then saves), or the save file says "session open" and the title warns about an improper exit.
pub static SESSION_EXIT: AtomicBool = AtomicBool::new(false);

/// Replaces the in-game update's call to OpenSession().
pub unsafe extern "system" fn open_session_hook() {
    if SESSION_EXIT.load(Relaxed) { return; }
    let f: unsafe extern "system" fn() = core::mem::transmute(BASE.load(Relaxed) + o::FN_OPEN_SESSION);
    f();
}

/// Request the session-end save (bits 0x201, what the system menu requests when it ends a session).
pub unsafe fn queue_session_end_save(base: usize) {
    let st = mem::rd_usize(base + o::SAVE_STATE_GLOBAL);
    if st == 0 || !mem::is_readable(st + o::SAVE_REQ_MASK, 4) { return; }
    let mask = &*((st + o::SAVE_REQ_MASK) as *const std::sync::atomic::AtomicU32);
    mask.fetch_or(o::SAVE_REQ_SESSION_END, std::sync::atomic::Ordering::SeqCst);
}

/// Mark the session closed the way the system menu's Return to Title does: "session open" byte cleared, GameMan's
/// proper-exit byte set. The save file only gets it with a session-end save (`request_session_end_save`). Plain
/// writes, any thread.
pub unsafe fn clear_session_flag(base: usize) {
    let st = mem::rd_usize(base + o::SAVE_STATE_GLOBAL);
    if st != 0 && mem::is_readable(st + o::SAVE_SESSION_OPEN, 1) { mem::wr_u8(st + o::SAVE_SESSION_OPEN, 0); }
    let gm = mem::rd_usize(base + o::GAME_MAN_GLOBAL);
    if gm != 0 && mem::is_readable(gm + o::GAME_MAN_PROPER_EXIT, 1) { mem::wr_u8(gm + o::GAME_MAN_PROPER_EXIT, 1); }
}
