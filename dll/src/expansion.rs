//! Core expansions (ASSAULT ARMOR, PULSE ARMOR, PULSE PROTECTION, TERMINAL ARMOR) as mission rewards. Assemblage's
//! EXTENSION list shows an expansion only when the inventory holds its item AND its OS Tuning level is above 0
//! (i32 per tuning at [GameData+0x1368], RE_NOTES "Core expansions"): the item alone stays hidden, the level alone
//! shows nothing. A reward queues the item in the pending list like any part; just before the game commits that list
//! (commit hook, game thread, after the reward-choice prune) the level of every expansion still in it is set to 1,
//! so a part not picked, or a sortie failed, unlocks nothing. Unique: never rolled while owned or unlocked.

use super::*;

/// equipId -> offset of its OS Tuning level (TGA table order; Pulse Armor verified live)
pub fn level_offset(equip_id: u32) -> Option<usize> {
    o::EXPANSION_LEVELS.iter().find(|e| e.0 == equip_id).map(|e| e.1)
}

unsafe fn levels(gd: usize) -> Option<usize> {
    let p = mem::rd_usize(gd + o::GD_OS_TUNING);
    if p == 0 || !mem::is_readable(p, 0x40) { return None; }
    Some(p)
}

/// OS Tuning level of an expansion (None: not an expansion, or the tuning data is not loaded).
pub unsafe fn level(gd: usize, equip_id: u32) -> Option<i32> {
    Some(mem::rd_i32(levels(gd)? + level_offset(equip_id)?))
}

/// The expansions of the current sortie's rewards: (equipId, pending-list handle, 0 = not found).
static QUEUED: std::sync::Mutex<Vec<(u32, u32)>> = std::sync::Mutex::new(Vec::new());

pub fn arm(v: Vec<(u32, u32)>) {
    if !v.is_empty() { logf!("expansion reward(s) queued: {:X?}", v); }
    if let Ok(mut q) = QUEUED.lock() { *q = v; }
}
pub fn disarm() { if let Ok(mut q) = QUEUED.lock() { q.clear(); } }

/// The game is about to commit the pending list (mgr = its item manager): unlock the queued expansions still in it.
pub unsafe fn commit(mgr: usize) {
    let queued = match QUEUED.lock() { Ok(mut q) => core::mem::take(&mut *q), Err(_) => return };
    if queued.is_empty() { return; }
    let gd = mgr - o::GD_ITEM_MGR;
    let (b, e) = (mem::rd_usize(mgr + o::MGR_PENDING_BEGIN), mem::rd_usize(mgr + o::MGR_PENDING_END));
    let pending: Vec<u32> = if b == 0 || e < b || e - b > 0x1000 { Vec::new() } else { (0..(e - b) / 4).map(|i| mem::rd_u32(b + 4 * i)).collect() };
    let Some(lv) = levels(gd) else { log("WARNING: expansion: OS Tuning data unreadable at the commit"); return };
    for (id, h) in queued {
        let Some(off) = level_offset(id) else { continue };
        if h == 0 { logf!("WARNING: expansion {}: no pending handle, not unlocked", id); continue; }
        if !pending.contains(&h) { logf!("expansion {}: not committed (not picked)", id); continue; }
        let old = mem::rd_i32(lv + off);
        if old < 1 { mem::wr_i32(lv + off, 1); }
        logf!("expansion {} ({}): OS Tuning level {} -> {} at the commit", id, penalty::part_name(id), old, old.max(1));
    }
}
