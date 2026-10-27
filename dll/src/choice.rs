//! Reward choice (pick 1 of K): every reward of a cleared mission offers `reward_choices` parts and the player picks
//! one in the native grid (rewardmenu.rs). The offers come from their own pool (every offered part leaves it, picked
//! or not) and their own random generator; both, and the pending offer, live in the save (event flags), so quitting
//! or reloading neither rerolls nor loses an offer.

use super::*;

// ---- saved state (free flag ranges of blocks 4 / 7, RE_NOTES "Save storage")
/// "Offered as a reward" row bitset, split over two free ranges (318 rows).
fn rolled_flag(row: u16) -> u32 {
    if (row as u32) < o::REWARD_ROLLED_A_LEN { o::REWARD_ROLLED_A + row as u32 } else { o::REWARD_ROLLED_B + row as u32 - o::REWARD_ROLLED_A_LEN }
}
unsafe fn rolled(g: &Game, man: usize, row: u16) -> bool { g.get_flag(man, rolled_flag(row)) == Some(true) }
unsafe fn set_rolled(g: &Game, man: usize, row: u16, v: bool) { g.put_flag(man, rolled_flag(row), v); }

unsafe fn load_rng(g: &Game, man: usize) -> Rng {
    let s = g.get_bits(man, o::REWARD_RNG, 32) as u64 | (g.get_bits(man, o::REWARD_RNG + 32, 32) as u64) << 32;
    if s == 0 { Rng::seeded() } else { Rng(s) }
}
unsafe fn save_rng(g: &Game, man: usize, r: &Rng) {
    g.put_bits(man, o::REWARD_RNG, 32, r.0 as u32);
    g.put_bits(man, o::REWARD_RNG + 32, 32, (r.0 >> 32) as u32);
}

/// The pending offer (lineup row indices), empty when none.
pub unsafe fn offer(g: &Game, man: usize) -> Vec<u16> {
    let n = g.get_bits(man, o::OFFER_COUNT, 4).min(12);
    (0..n).map(|k| g.get_bits(man, o::OFFER_ROWS + 9 * k, 9) as u16).collect()
}
unsafe fn write_offer(g: &Game, man: usize, rows: &[u16]) {
    for (k, r) in rows.iter().enumerate().take(12) { g.put_bits(man, o::OFFER_ROWS + 9 * k as u32, 9, *r as u32); }
    g.put_bits(man, o::OFFER_COUNT, 4, rows.len().min(12) as u32);
}
/// Rewards still to pick for the last cleared mission, the pending offer included.
pub unsafe fn sets_left(g: &Game, man: usize) -> u32 { g.get_bits(man, o::OFFER_SETS_LEFT, 4) }
unsafe fn put_sets_left(g: &Game, man: usize, n: u32) { g.put_bits(man, o::OFFER_SETS_LEFT, 4, n.min(15)); }

/// Roll one offer of `k` parts by rarity: not owned, not lost forever, not offered before. A pool that runs dry is
/// refilled once (everything but this roll) and completes from the refill. Every offered part leaves the pool.
unsafe fn roll(g: &Game, gd: usize, man: usize, pool: &[Part], k: usize) -> Vec<Part> {
    let mut rng = load_rng(g, man);
    let owned = g.owned_ids(gd);
    let lost = g.read_rows(man, o::ROWS_LOST);
    let mut out: Vec<Part> = Vec::new();
    let mut refilled = false;
    while out.len() < k {
        let cands: Vec<Part> = pool.iter().copied().filter(|p| !rolled(g, man, p.idx) && !lost.contains(&p.idx)
            && !owned.contains(&p.grant_id()) && !out.iter().any(|o| o.idx == p.idx)).collect();
        match pick_weighted(&mut rng, &cands) {
            Some(p) => out.push(p),
            None if !refilled => {
                let mut n = 0;
                for p in pool { if rolled(g, man, p.idx) && !out.iter().any(|o| o.idx == p.idx) { set_rolled(g, man, p.idx, false); n += 1; } }
                logf!("reward pool refill: {} part(s) back in", n);
                refilled = true;
            }
            None => break,
        }
    }
    for p in &out { set_rolled(g, man, p.idx, true); }
    save_rng(g, man, &rng);
    out
}

/// Grant ids of an offer's rows (rows not in the pool are skipped).
fn ids(pool: &[Part], rows: &[u16]) -> Vec<u32> {
    rows.iter().filter_map(|r| pool.iter().find(|p| p.idx == *r)).map(|p| p.grant_id()).collect()
}

/// Ask the game thread to open the grid for the pending offer.
unsafe fn show(pool: &[Part], rows: &[u16]) {
    let v = ids(pool, rows);
    for (k, p) in rewardmenu::PARTS.iter().enumerate() { p.store(v.get(k).copied().unwrap_or(0), Relaxed); }
    rewardmenu::VTABLE.store(rewardmenu::OWN as u32, Relaxed);
    rewardmenu::OPEN_REQUEST.store(1, Relaxed);
    logf!("reward choice: offer {:X?} opened", v);
}

/// Mission cleared (flag writes only, before the commit autosave): roll the first offer and open it. An empty roll
/// (nothing left that is not owned / lost) returns false: the caller pays the reward in credits instead.
pub unsafe fn on_mission_cleared(g: &Game, gd: usize, man: usize, pool: &[Part]) -> bool {
    let cfg = config::get();
    if cfg.parts_per_mission == 0 { return true; }
    let rolled = roll(g, gd, man, pool, cfg.reward_choices as usize);
    logf!("reward choice: {} reward(s), first offer rows {:?}", cfg.parts_per_mission, rolled.iter().map(|p| p.idx).collect::<Vec<_>>());
    if rolled.is_empty() { return false; }
    let rows: Vec<u16> = rolled.iter().map(|p| p.idx).collect();
    write_offer(g, man, &rows);
    put_sets_left(g, man, cfg.parts_per_mission);
    show(pool, &rows);
    true
}

/// A save was loaded with an offer still pending (quit before picking): show it again.
pub unsafe fn on_save_loaded(g: &Game, man: usize, pool: &[Part]) {
    let rows = offer(g, man);
    if !rows.is_empty() { logf!("reward choice: pending offer {:?} found in the save", rows); show(pool, &rows); }
}

/// The player picked `id` (DLL loop): grant it, then roll the next offer or close the reward. Returns true when a
/// save should be requested (picked in the garage: no mission commit will save it).
pub unsafe fn on_picked(g: &Game, gd: usize, man: usize, pool: &[Part], id: u32) -> bool {
    let in_mission = g.defer_byte(gd) == 1;
    if in_mission {
        let (r, err) = g.give_deferred(gd, id);
        logf!("reward choice: {:#X} queued with the mission (rax={} err={})", id, r, err);
    } else {
        penalty::GIVE_REQUEST.store(id, Relaxed);
        logf!("reward choice: {:#X} granted in the garage", id);
    }
    let left = sets_left(g, man).saturating_sub(1);
    put_sets_left(g, man, left);
    write_offer(g, man, &[]);
    if left > 0 {
        let next = roll(g, gd, man, pool, config::get().reward_choices as usize);
        if !next.is_empty() {
            let rows: Vec<u16> = next.iter().map(|p| p.idx).collect();
            write_offer(g, man, &rows);
            show(pool, &rows);
        } else { put_sets_left(g, man, 0); }
    }
    !in_mission
}
