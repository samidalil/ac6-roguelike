//! Reward choice (pick 1 of K): each of a mission's rewards (parts_per_mission) offers `reward_choices` parts. The
//! offers are rolled and queued at sortie like plain rewards (the game's pending obtained-items list); at the results
//! screen the game's "Pièces obtenues" opens our grid instead (rewardmenu.rs, one page per reward), and just before the
//! game commits the pending list the parts not picked are taken out of it. The offers come from their own pool (every
//! offered part leaves it when the mission is cleared, picked or not) and their own random generator, both saved in
//! event flags.

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

/// Roll `n` offers of `k` parts by rarity: not owned, not lost forever, not offered before, each part in one offer
/// only. A pool that runs dry is refilled once (everything but this roll) and completes from the refill; an offer
/// still empty after that is left out (the caller pays that reward in credits).
pub unsafe fn roll(g: &Game, gd: usize, man: usize, pool: &[Part], n: u32, k: usize) -> Vec<Vec<Part>> {
    let mut rng = load_rng(g, man);
    let owned = g.owned_ids(gd);
    let lost = g.read_rows(man, o::ROWS_LOST);
    let mut sets: Vec<Vec<Part>> = Vec::new();
    let mut taken: Vec<u16> = Vec::new();
    let mut refilled = false;
    for _ in 0..n {
        let mut set = Vec::new();
        while set.len() < k {
            let cands: Vec<Part> = pool.iter().copied().filter(|p| !rolled(g, man, p.idx) && !lost.contains(&p.idx)
                && !owned.contains(&p.grant_id()) && !taken.contains(&p.idx)).collect();
            match pick_weighted(&mut rng, &cands) {
                Some(p) => { taken.push(p.idx); set.push(p); }
                None if !refilled => {
                    let mut m = 0;
                    for p in pool { if rolled(g, man, p.idx) && !taken.contains(&p.idx) { set_rolled(g, man, p.idx, false); m += 1; } }
                    logf!("reward pool refill: {} part(s) back in", m);
                    refilled = true;
                }
                None => break,
            }
        }
        if set.is_empty() { break; }
        sets.push(set);
    }
    save_rng(g, man, &rng);
    sets
}

/// Mission cleared: every offered part leaves the reward pool.
pub unsafe fn mark_offered(g: &Game, man: usize, parts: &[Part]) {
    for p in parts { set_rolled(g, man, p.idx, true); }
}

/// The current sortie's offers, by reward: (grant id, pending-list handle, 0 = not found) of each part.
static ARMED: std::sync::Mutex<Vec<Vec<(u32, u32)>>> = std::sync::Mutex::new(Vec::new());

/// The offers are queued (sortie, or re-queued after a restart).
pub fn arm(sets: Vec<Vec<(u32, u32)>>) {
    logf!("reward choice armed: {:X?}", sets);
    if let Ok(mut a) = ARMED.lock() { *a = sets; }
}
pub fn disarm() { if let Ok(mut a) = ARMED.lock() { a.clear(); } }

/// Results screen: the pages of our grid for the game's pending parts list `ids` (None = no choice armed: the vanilla
/// dialog). One page per offer, then one per other part of the list (a mission's own reward).
pub fn pages(ids: &[u32]) -> Option<Vec<rewardmenu::Page>> {
    let sets = ARMED.lock().ok()?.clone();
    if sets.is_empty() { return None; }
    let n = sets.len();
    let mut pages: Vec<rewardmenu::Page> = sets.iter().enumerate().map(|(i, s)| rewardmenu::Page {
        ids: s.iter().map(|x| x.0).collect(),
        title: if n > 1 { format!("CHOISISSEZ UNE PIÈCE ({}/{})", i + 1, n) } else { "CHOISISSEZ UNE PIÈCE".to_string() },
    }).collect();
    for id in ids {
        if !sets.iter().flatten().any(|x| x.0 == *id) { pages.push(rewardmenu::Page { ids: vec![*id], title: "PIÈCE OBTENUE".to_string() }); }
    }
    Some(pages)
}

/// The game is about to commit the pending list (mgr = its item manager): take the parts not picked out of it.
pub unsafe fn prune(mgr: usize) {
    let sets = match ARMED.lock() { Ok(mut a) => core::mem::take(&mut *a), Err(_) => return };
    if sets.is_empty() { return; }
    let mut drop = Vec::new();
    let mut kept = Vec::new();
    for (s, set) in sets.iter().enumerate() {
        let pick = rewardmenu::picked(s).min(set.len() - 1);
        for (j, (id, h)) in set.iter().enumerate() {
            if j == pick { kept.push(*id); } else if *h != 0 { drop.push(*h); } else { logf!("WARNING: reward choice: no handle for {:#X}, it stays granted", id); }
        }
    }
    let (b, e) = (mem::rd_usize(mgr + o::MGR_PENDING_BEGIN), mem::rd_usize(mgr + o::MGR_PENDING_END));
    if b == 0 || e < b || e - b > 0x1000 { log("WARNING: reward choice: pending list unreadable at the commit"); return; }
    let all: Vec<u32> = (0..(e - b) / 4).map(|i| mem::rd_u32(b + 4 * i)).collect();
    let keep: Vec<u32> = all.iter().copied().filter(|h| !drop.contains(h)).collect();
    for (i, h) in keep.iter().enumerate() { mem::wr_u32(b + 4 * i, *h); }
    core::ptr::write_unaligned((mgr + o::MGR_PENDING_END) as *mut usize, b + 4 * keep.len());
    logf!("reward choice: picked {:X?}; pending {:X?} -> {:X?} at the commit", kept, all, keep);
}
