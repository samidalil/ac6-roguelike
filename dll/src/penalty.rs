//! Failure penalty + negative-balance auto-sell, executed on the GAME THREAD.
//!
//! Game code (remove item, give item) must not be called from the DLL's own thread (that froze the
//! game once). We detour the per-frame save step call at exe+1274418 to `frame_hook`, which runs the
//! original step and then any work the DLL thread queued through the atomics below.
//!
//! Rules (RE_NOTES "Next features" item 3):
//! * every equipped weapon is lost, its slot left empty;
//! * every equipped frame part / internal is lost and its slot gets the most recently obtained spare for it, else the
//!   starter, bought at its shop price; tank legs take no booster;
//! * `[lost_on_failure]` changes that per item (0 = survives), and with a 1 the equipped core expansion is lost too:
//!   item removed, its OS Tuning level back to 0; a listed non-expansion OS Tuning (no slot: every installed one)
//!   loses one level;
//! * lost boosters / generators / FCS (starters excepted) are lost forever: never rolled again. Weapons and frame
//!   parts can be owned several times, so a lost one may roll again. A loss does not change `[max_occurrences]` counts;
//! * a negative balance sells stash parts (never equipped ones), most expensive first; still negative with
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
/// DEBUG: an item id (tag | equipId) to grant in the garage, 0 = none.
pub static GIVE_REQUEST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// DEBUG: grant GIVE_REQUEST even when it is already owned (a second copy).
pub static GIVE_FORCE: AtomicBool = AtomicBool::new(false);


static BASE: AtomicUsize = AtomicUsize::new(0);
static CTX: OnceLock<Ctx> = OnceLock::new();

pub struct Ctx {
    /// sell value by inventory item id (tag | equipId): part_/weapon_sell_percent of the part's value
    pub price_of: HashMap<u32, i32>,
    /// starter price per frame/internal slot (index into FRAME_SLOTS)
    pub starter_price: [i32; 7],
    /// equipIds of the tank legs (no booster slot)
    pub tank_legs: Vec<u32>,
    /// [lost_on_failure] by inventory item id (tag | equipId; core expansions: their plain item id)
    pub lost_rule: HashMap<u32, bool>,
    /// non-expansion OS Tunings (OsReinforce rows) losing one level on a failed sortie ([lost_on_failure] = 1)
    pub os_lost: Vec<u16>,
}

impl Ctx {
    /// Is the equipped item `item` lost on a failed sortie? Unlisted: yes, except core expansions.
    fn lost_on_failure(&self, item: u32) -> bool {
        self.lost_rule.get(&item).copied().unwrap_or_else(|| os::of_item(item).is_none())
    }
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
/// Equipped core expansion: assembly slot 11 (ID array, plain item id 75xxxxxx; index / handle at the same deltas).
pub const EXPANSION_SLOT: usize = 0x520;
/// left arm, right arm, left back, right back
pub const WEAPON_SLOTS: [(usize, &str); 4] = [(0x4F4, "L-arm"), (0x4F8, "R-arm"), (0x4FC, "L-back"), (0x500, "R-back")];
const EMPTY: u32 = 0xFFFF_FFFF;
const LEGS: usize = 3;
const BOOSTER: usize = 4;

/// FRAME_SLOTS index of an inventory item id (tag | equipId), None for weapons and the rest.
fn frame_slot(item: u32) -> Option<usize> {
    match (item >> 28, (item & 0x0FFF_FFFF) / 1_000_000) {
        (1, 50) => Some(0), (1, 51) => Some(1), (1, 52) => Some(2), (1, 53) => Some(LEGS),
        (6, _) => Some(BOOSTER), (5, _) => Some(5), (7, _) => Some(6), _ => None,
    }
}
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
/// price is rewritten to that cost, and a failure charges it for each starter it buys.
pub unsafe fn init(base: usize, lu: &Lineup, tank_legs: Vec<u32>) {
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
        // shop buy price = the charge (only rows the shop can sell: priced by the game, given a value, or a starter:
        // starters are always buyable)
        let starter = FRAME_SLOTS.iter().any(|s| s.3 | s.2 == item);
        if price > 0 || over.is_some() || starter { mem::wr_i32(lu.row(i) + o::ROW_PRICE, pct(value, charge_pct)); }
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
    logf!("tank legs (no booster): {:?}", tank_legs);
    let (lost_rule, os_lost) = lost_rule(lu);
    let _ = CTX.set(Ctx { price_of, starter_price, tank_legs, lost_rule, os_lost });
}

/// [lost_on_failure] entries resolved to item ids: every lineup weapon / frame part / internal (by equipId or name)
/// and the core expansions (OsReinforce row, item id or either name); plus the other OS Tunings listed with a 1 (row
/// or name), which lose one level.
unsafe fn lost_rule(lu: &Lineup) -> (HashMap<u32, bool>, Vec<u16>) {
    let table = &config::get().lost_on_failure;
    let (mut m, mut os_lost) = (HashMap::new(), Vec::new());
    if table.is_empty() { return (m, os_lost); }
    for i in 0..o::ROW_COUNT {
        let (id, kind) = (lu.equip_id(i), lu.equip_type(i));
        if !matches!(kind, 0 | 1 | 4 | 5 | 6) || os::of_item(id).is_some() { continue; }
        let item = o::id_tag(kind) | (id & 0x0FFF_FFFF);
        if let Some(v) = config::lookup(table, &[id], &[part_name(item)]) { m.insert(item, v); }
    }
    for t in os::tunings() {
        let Some(v) = config::lookup(table, &[t.row as u32, t.item], &[os::name(t.row), part_name(t.item)]) else { continue };
        if t.item == 0 { if v { os_lost.push(t.row); } continue; }
        m.insert(t.item, v);
    }
    let mut v: Vec<String> = m.iter().map(|(k, v)| format!("{} = {}", part_name(*k), *v as u8)).collect();
    v.sort();
    logf!("[lost_on_failure]: {}", v.join(", "));
    logf!("[lost_on_failure]: OS Tunings losing one level: {:?}", os_lost);
    (m, os_lost)
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
    let Some(gd) = game.game_data_fast() else { return };
    rewardmenu::tick(base);
    cap::tick_any();
    script::run_queue(); // chip scripts' handlers of the events emitted since the last frame
    event::apply_speffects(base); // SpEffects queued for the player (event actions, scripts, debug)
    if RETIRE_REQUEST.swap(false, Relaxed) && game.defer_byte(gd) == 1 {
        let retire: unsafe extern "system" fn() -> u8 = core::mem::transmute(base + o::FN_RETIRE_MISSION);
        retire();
        RETIRED.fetch_add(1, Relaxed);
        return;
    }
    rewardmenu::open_pending(base); // movie name restore (also during the results)
    if game.defer_byte(gd) != 0 { return; } // the rest is garage-only work
    dialog::open_pending(base);
    if CLEANUP_REQUEST.swap(false, Relaxed) { cleanup_orphans(&game, gd); }
    // mission_failed first: the chips lost by the penalty were still installed during the failed mission
    if PENALTY_REQUEST.swap(false, Relaxed) { event::emit(event::Event::MissionFailed); apply_penalty(&game, ctx, gd); }
    cap::tick(gd);
    let give = GIVE_REQUEST.swap(0, Relaxed);
    if give != 0 {
        if GIVE_FORCE.swap(false, Relaxed) {
            let mut buf = [0u8; 0x40];
            let d = game.desc(give, &mut buf);
            let r = (game.give_item)(gd + o::GD_GIVE_OBJ, d, 1, 1, 1, 1, 1);
            logf!("DEBUG: give2 {:#X}: rax={} status={}", give, r, mem::rd_u32(gd + o::GD_GIVE_OBJ + o::GIVEOBJ_ERROR));
            log_duplicates(gd);
        } else { logf!("DEBUG: give {:#X}: granted {}", give, ensure_owned(&game, gd, give)); }
    }
    let deficit = AUTOSELL_DEFICIT.swap(0, Relaxed);
    if deficit < 0 { auto_sell(&game, ctx, gd, deficit); event::emit(event::Event::AutoSell); }
    event::run_garage(&game, gd); // credits / give queued by event actions
    event::open_dialog();
}

// ---------------------------------------------------------------- inventory helpers
/// Address of inventory entry `i` ({handle, id, count, ordinal}); the first [mgr+1C] entries live at
/// [mgr+40], the rest at [mgr+30].
pub unsafe fn entry(mgr: usize, i: u32) -> usize {
    let split = mem::rd_u32(mgr + o::MGR_TABLE_SPLIT);
    if i < split { mem::rd_usize(mgr + o::MGR_TABLE_LO) + i as usize * 16 }
    else { mem::rd_usize(mgr + o::MGR_TABLE_HI) + (i - split) as usize * 16 }
}
pub unsafe fn capacity(mgr: usize) -> u32 { mem::rd_u32(mgr + o::MGR_TABLE_COUNT).min(4096) }

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

/// An inventory item id (tag | equipId) by part name or equipId, among the priced parts.
pub fn item_by_name(name: &str) -> Option<u32> {
    let ctx = CTX.get()?;
    let n = name.trim();
    if let Ok(id) = n.parse::<u32>() { return ctx.price_of.keys().copied().find(|k| k & 0x0FFF_FFFF == id); }
    let k = config::name_key(n);
    ctx.price_of.keys().copied().find(|&id| config::name_key(&part_name(id)) == k)
}

/// Grant one copy of `id` in the garage (game thread). A booster / generator / FCS (one copy at most) only if not
/// owned. True when granted.
pub unsafe fn give_copy(game: &Game, gd: usize, id: u32) -> bool {
    if matches!(id >> 28, 5 | 6 | 7) { return ensure_owned(game, gd, id); }
    let mut buf = [0u8; 0x40];
    let d = game.desc(id, &mut buf);
    (game.give_item)(gd + o::GD_GIVE_OBJ, d, 1, 1, 1, 1, 1);
    true
}

/// Make sure the player owns `id`; grants it directly (garage: no deferral) if missing.
unsafe fn ensure_owned(game: &Game, gd: usize, id: u32) -> bool {
    if find(gd + o::GD_ITEM_MGR, id).is_some() { return false; }
    let mut buf = [0u8; 0x40];
    let d = game.desc(id, &mut buf);
    (game.give_item)(gd + o::GD_GIVE_OBJ, d, 1, 1, 1, 1, 1);
    true
}

/// DEBUG: log the inventory entries (index, handle, count, ordinal, equipped) of every item id owned more than once.
pub unsafe fn log_duplicates(gd: usize) {
    let mgr = gd + o::GD_ITEM_MGR;
    let equipped = equipped_handles(gd);
    let mut by_id: Vec<(u32, Vec<String>)> = Vec::new();
    for i in 0..capacity(mgr) {
        let e = entry(mgr, i);
        let (h, id, n, ord) = (mem::rd_u32(e), mem::rd_u32(e + 4), mem::rd_u32(e + 8), mem::rd_u32(e + 0xC));
        if h == EMPTY || id >> 28 == 4 { continue; }
        let s = format!("#{} h={:#X} n={} ord={}{}", i, h, n, ord, if equipped.contains(&h) { " EQ" } else { "" });
        match by_id.iter_mut().find(|x| x.0 == id) { Some(x) => x.1.push(s), None => by_id.push((id, vec![s])) }
    }
    let dups: Vec<_> = by_id.iter().filter(|x| x.1.len() > 1).collect();
    logf!("DEBUG: {} part id(s), {} owned more than once", by_id.len(), dups.len());
    for (id, v) in dups { logf!("DEBUG:   {:#X} {}: {}", id, part_name(*id), v.join(" | ")); }
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
/// Parts the player can sell right now (unequipped, priced): cap.rs only opens Vente when there is one.
pub unsafe fn sellable_count(gd: usize) -> usize { CTX.get().map_or(0, |ctx| stash(ctx, gd).len()) }

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
/// What fills a frame slot after its part is lost.
#[derive(Clone, Copy)]
enum Fill {
    /// an owned spare: (handle, item id)
    Spare(u32, u32),
    /// the starter, bought at its shop price
    Buy,
    /// nothing: booster slot under tank legs
    Empty,
    /// the equipped part survives ([lost_on_failure] = 0)
    Keep,
}

/// Most recently obtained owned, unequipped part for frame slot `k`: highest inventory ordinal (entry +0xC), then the
/// most valuable. Returns (handle, item id).
unsafe fn newest_spare(ctx: &Ctx, mgr: usize, equipped: &[u32], k: usize) -> Option<(u32, u32)> {
    (0..capacity(mgr)).map(|i| entry(mgr, i))
        .map(|e| (mem::rd_u32(e), mem::rd_u32(e + 4), mem::rd_u32(e + 0xC)))
        .filter(|&(h, id, _)| h != EMPTY && !equipped.contains(&h) && frame_slot(id) == Some(k))
        .max_by_key(|&(_, id, ord)| (ord, ctx.price_of.get(&id).copied().unwrap_or(0)))
        .map(|(h, id, _)| (h, id))
}

/// Everything a failed sortie costs, computed once when the sortie is committed (deployment talk). The loadout
/// and inventory cannot change during a mission, and the escrow save is written at that same moment, so a plan
/// rebuilt from that save on load is identical. The death check and the garage penalty both use this plan.
/// Single pass: replacements, then the starters' cost, then the sales (which never touch the replacements).
pub struct FailurePlan {
    /// weapon slots emptied: (id offset in PlayerGameData, slot name, handle, id)
    weapons: Vec<(usize, &'static str, u32, u32)>,
    /// every frame slot: (FRAME_SLOTS index, lost (handle, id) if the slot held a part, what fills it)
    frames: Vec<(usize, Option<(u32, u32)>, Fill)>,
    /// the equipped core expansion, when [lost_on_failure] says it is lost: (OsReinforce row, handle, item id)
    expansion: Option<(u16, u32, u32)>,
    /// installed OS Tunings losing one level: (OsReinforce row, level at sortie)
    os_levels: Vec<(u16, i32)>,
    starter_cost: i64,
    /// stash parts sold to cover the cost, most expensive first (the spares about to be equipped excluded)
    sell: Vec<(i32, u32, u32)>,
}

impl FailurePlan {
    unsafe fn build(ctx: &Ctx, gd: usize) -> FailurePlan {
        let mgr = gd + o::GD_ITEM_MGR;
        let equipped = equipped_handles(gd);
        let mut sell = stash(ctx, gd);
        // weapon slot ids hold the plain equipId (tag 0)
        let weapons = WEAPON_SLOTS.iter().filter_map(|&(off, name)| {
            let id = mem::rd_u32(gd + off);
            if id == EMPTY || !ctx.lost_on_failure(id) { None } else { Some((off, name, mem::rd_u32(gd + off - HANDLE_DELTA), id)) }
        }).collect();
        let (mut frames, mut starter_cost, mut tank) = (Vec::new(), 0i64, false);
        for (k, (off, _, starter, tag)) in FRAME_SLOTS.iter().enumerate() {
            let id = mem::rd_u32(gd + off);
            let keep = id != EMPTY && !ctx.lost_on_failure(tag | id);
            let lost = if id == EMPTY || keep { None } else { Some((mem::rd_u32(gd + off - HANDLE_DELTA), id)) };
            let fill = if k == BOOSTER && tank { Fill::Empty } else if keep { Fill::Keep } else {
                match newest_spare(ctx, mgr, &equipped, k) {
                    Some((h, item)) => { sell.retain(|s| s.1 != h); Fill::Spare(h, item) }
                    None => { starter_cost += ctx.starter_price[k] as i64; Fill::Buy }
                }
            };
            if k == LEGS {
                let legs = match fill { Fill::Spare(_, item) => item & 0x0FFF_FFFF, Fill::Keep => id, _ => *starter };
                tank = ctx.tank_legs.contains(&legs);
            }
            frames.push((k, lost, fill));
        }
        let x = mem::rd_u32(gd + EXPANSION_SLOT);
        let expansion = if x == EMPTY { None } else {
            os::of_item(x).filter(|t| ctx.lost_on_failure(t.item)).map(|t| (t.row, mem::rd_u32(gd + EXPANSION_SLOT - HANDLE_DELTA), t.item))
        };
        let os_levels = ctx.os_lost.iter().filter_map(|&row| Some((row, os::level(gd, row).filter(|&l| l > 0)?))).collect();
        FailurePlan { weapons, frames, expansion, os_levels, starter_cost, sell }
    }

    /// Balance left after the penalty and selling everything sellable; negative = game over.
    pub fn balance(&self, coam: i64) -> i64 {
        coam - self.starter_cost + self.sell.iter().map(|s| s.0 as i64).sum::<i64>()
    }

    fn summary(&self) -> String {
        let n = |f: fn(&Fill) -> bool| self.frames.iter().filter(|x| f(&x.2)).count();
        format!("{} weapon(s) lost, {} part(s) lost, {} kept, expansion lost {:?}, OS levels lost {:?}, {} spare(s) equipped, {} starter(s) to buy ({}), {} slot(s) left empty, {} sellable part(s) worth {}",
            self.weapons.len(), self.frames.iter().filter(|f| f.1.is_some()).count(), n(|f| matches!(f, Fill::Keep)), self.expansion.map(|e| e.0), self.os_levels,
            n(|f| matches!(f, Fill::Spare(..))), n(|f| matches!(f, Fill::Buy)), self.starter_cost, n(|f| matches!(f, Fill::Empty)),
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
/// Put inventory entry `index` in the slot whose id lives at `off` (index + handle + id: all three must agree).
unsafe fn equip(gd: usize, off: usize, index: u32, id: u32) {
    let mgr = gd + o::GD_ITEM_MGR;
    mem::wr_u32(gd + off - INDEX_DELTA, index);
    mem::wr_u32(gd + off - HANDLE_DELTA, mem::rd_u32(entry(mgr, index)));
    mem::wr_u32(gd + off, id);
}

unsafe fn unequip(gd: usize, off: usize) {
    for d in [INDEX_DELTA, HANDLE_DELTA, 0] { mem::wr_u32(gd + off - d, EMPTY); }
}

/// Booster, generator, FCS: one copy at most (copies would share a handle), lost forever on a failure.
pub fn unique_tag(item: u32) -> bool { matches!(item >> 28, 5 | 6 | 7) }

/// Hide a lost part from the pool for the rest of the run (unique parts only: a lost weapon / frame part may roll again).
unsafe fn mark_lost(game: &Game, item: u32) {
    if !unique_tag(item) { return; }
    let (Some(man), Some(row)) = (game.flag_man(), buyback::row_of(item)) else { return };
    game.put_row(man, o::HIDE_FROM_POOL, row, true);
}

unsafe fn apply_penalty(game: &Game, ctx: &Ctx, gd: usize) {
    let plan = PLAN.lock().unwrap().take();
    let plan = plan.unwrap_or_else(|| { log("penalty: no armed plan, building one now"); FailurePlan::build(ctx, gd) });
    let mgr = gd + o::GD_ITEM_MGR;
    let mut lines: Vec<String> = Vec::new();
    let mut sold: Vec<(u32, i32)> = Vec::new();
    // lost weapons leave their slot empty; empty slots are cleared too (drops a stale index, which would show
    // whatever part later takes that inventory index); kept weapons stay ([lost_on_failure] = 0)
    for &(off, _) in &WEAPON_SLOTS {
        if mem::rd_u32(gd + off) == EMPTY || plan.weapons.iter().any(|w| w.0 == off) { unequip(gd, off); }
    }
    for &(_, name, handle, id) in &plan.weapons {
        let removed = remove_handle(game, gd, handle, id);
        mark_lost(game, id);
        lines.push(format!("lost {} weapon {}{}", name, id, if removed { "" } else { " (was not in the inventory)" }));
    }
    // equip the replacement first, then remove the lost part
    for &(k, lost, fill) in &plan.frames {
        let (off, name, starter, tag) = FRAME_SLOTS[k];
        let mut keep = false;
        match fill {
            Fill::Spare(h, item) => {
                let Some(si) = find_handle(mgr, h) else { lines.push(format!("{}: spare {:#X} missing!", name, item)); continue };
                equip(gd, off, si, item & 0x0FFF_FFFF);
                lines.push(format!("{}: spare {:#X} equipped", name, item));
            }
            // losing the equipped starter and buying it back = keeping it
            Fill::Buy if lost.map(|l| l.1) == Some(starter) => {
                keep = true;
                lines.push(format!("{}: starter lost and bought back ({})", name, ctx.starter_price[k]));
            }
            Fill::Buy => {
                ensure_owned(game, gd, tag | starter);
                let Some(si) = find(mgr, tag | starter) else { lines.push(format!("{}: starter {} missing after grant!", name, starter)); continue };
                equip(gd, off, si, starter);
                lines.push(format!("{}: starter {} bought ({})", name, starter, ctx.starter_price[k]));
            }
            Fill::Empty => { unequip(gd, off); lines.push(format!("{}: left empty (tank legs)", name)); }
            Fill::Keep => { lines.push(format!("{}: kept ([lost_on_failure] = 0)", name)); }
        }
        if let (Some((h, id)), false) = (lost, keep) {
            remove_handle(game, gd, h, tag | id);
            if id != starter { mark_lost(game, tag | id); }
            lines.push(format!("{}: lost {}{}", name, id, if id != starter && unique_tag(tag) { " forever" } else { "" }));
        }
    }
    // the equipped core expansion ([lost_on_failure] = 1): slot emptied, item removed, OS Tuning level back to 0
    if let Some((row, handle, item)) = plan.expansion {
        unequip(gd, EXPANSION_SLOT);
        let removed = remove_handle(game, gd, handle, item);
        os::reset(gd, row);
        lines.push(format!("core expansion {} ({}) lost{}", os::name(row), row, if removed { "" } else { " (was not in the inventory)" }));
    }
    // the other listed OS Tunings: one level less each
    for &(row, _) in &plan.os_levels {
        if let Some((old, new)) = os::level_down(gd, row) { lines.push(format!("OS Tuning {} ({}): level {} -> {}", os::name(row), row, old, new)); }
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
    // a starter lost and bought back did not change: it only shows in the charge line (the box holds ~12 lines)
    let changed: Vec<_> = plan.frames.iter().filter(|f| !matches!(f.2, Fill::Keep) && !(matches!(f.2, Fill::Buy) && f.1.map(|l| l.1) == Some(FRAME_SLOTS[f.0].2))).collect();
    let lost: Vec<String> = changed.iter().filter_map(|f| f.1.map(|l| part_name(FRAME_SLOTS[f.0].3 | l.1))).collect();
    if !lost.is_empty() { t += &format!("Pièces perdues : {}\n", lost.join(", ")); }
    let repl: Vec<String> = changed.iter().filter_map(|f| match f.2 {
        Fill::Spare(_, item) => Some(part_name(item)),
        Fill::Buy => Some(part_name(FRAME_SLOTS[f.0].3 | FRAME_SLOTS[f.0].2)),
        Fill::Empty | Fill::Keep => None,
    }).collect();
    if !repl.is_empty() { t += &format!("Remplacées par : {}\n", repl.join(", ")); }
    if let Some((row, ..)) = plan.expansion { t += &format!("Extension perdue : {}\n", os::name(row)); }
    if !plan.os_levels.is_empty() {
        let names: Vec<String> = plan.os_levels.iter().map(|&(row, lv)| format!("{} (LV {} > {})", os::name(row), lv, lv - 1)).collect();
        t += &format!("Niveaux OS perdus : {}\n", names.join(", "));
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
pub fn money(v: i64) -> String {
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
            buyback::record(id, "auto-sell");
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
