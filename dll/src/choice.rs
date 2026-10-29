//! Reward choice (pick 1 of K): each of a mission's rewards (parts_per_mission) offers `reward_choices` parts. The
//! offers are rolled and queued at sortie like plain rewards (the game's pending obtained-items list); at the results
//! screen the game's "Pièces obtenues" opens our grid instead (rewardmenu.rs, one page per reward), and just before the
//! game commits the pending list the parts not picked are taken out of it. The offers are drawn by rarity from the
//! reward candidates (lib.rs `candidates`) with their own random generator, saved in event flags.

use super::*;

// ---- saved state (free flag range of block 4, RE_NOTES "Save storage")
unsafe fn load_rng(g: &Game, man: usize) -> Rng {
    let s = g.get_bits(man, o::REWARD_RNG, 32) as u64 | (g.get_bits(man, o::REWARD_RNG + 32, 32) as u64) << 32;
    if s == 0 { Rng::seeded() } else { Rng(s) }
}
unsafe fn save_rng(g: &Game, man: usize, r: &Rng) {
    g.put_bits(man, o::REWARD_RNG, 32, r.0 as u32);
    g.put_bits(man, o::REWARD_RNG + 32, 32, (r.0 >> 32) as u32);
}

/// Roll `n` offers of `k` parts by rarity among `cands`, each part in one offer only. An offer left empty (fewer
/// candidates than offered parts: only with a tiny pool) is left out.
pub unsafe fn roll(g: &Game, man: usize, cands: &[Part], n: u32, k: usize) -> Vec<Vec<Part>> {
    let mut rng = load_rng(g, man);
    let mut sets: Vec<Vec<Part>> = Vec::new();
    let mut taken: Vec<u16> = Vec::new();
    for _ in 0..n {
        let mut set = Vec::new();
        while set.len() < k {
            let left: Vec<Part> = cands.iter().copied().filter(|p| !taken.contains(&p.idx)).collect();
            let Some(p) = pick_weighted(&mut rng, &left) else { break };
            taken.push(p.idx);
            set.push(p);
        }
        if set.is_empty() { break; }
        sets.push(set);
    }
    save_rng(g, man, &rng);
    sets
}

/// One offered part: the item queued in the pending list (`id`, its pending `handle`, 0 = not found), and for an OS
/// Tuning offer its OsReinforce row, whether the item is kept when picked (a core expansion not owned yet; else a
/// carrier, always taken out) and its tile texts (name, category + level); `pool` = its pool's tile label (text,
/// 0xRRGGBB), None when its pool shows none.
#[derive(Clone, Debug)]
pub struct Offer { pub id: u32, pub handle: u32, pub os: u16, pub keep: bool, pub label: Option<(String, String)>, pub pool: Option<(String, u32)> }

/// The current sortie's offers, by reward.
static ARMED: std::sync::Mutex<Vec<Vec<Offer>>> = std::sync::Mutex::new(Vec::new());

/// The offers are queued (sortie, or re-queued after a restart).
pub fn arm(sets: Vec<Vec<Offer>>) {
    logf!("reward choice armed: {:X?}", sets.iter().map(|s| s.iter().map(|x| (x.id, x.handle, x.os)).collect::<Vec<_>>()).collect::<Vec<_>>());
    if let Ok(mut a) = ARMED.lock() { *a = sets; }
}
pub fn disarm() { if let Ok(mut a) = ARMED.lock() { a.clear(); } }

/// Results screen: the pages of our grid for the game's pending parts list `ids` (None = no choice armed: the vanilla
/// dialog). One page per offer, then one per other part of the list (a mission's own reward).
pub fn pages(ids: &[u32]) -> Option<Vec<rewardmenu::Page>> {
    let sets = ARMED.lock().ok()?.clone();
    if sets.is_empty() { return None; }
    let n = sets.iter().filter(|s| s.len() > 1).count();
    let mut k = 0;
    let mut pages: Vec<rewardmenu::Page> = sets.iter().map(|s| rewardmenu::Page {
        ids: s.iter().map(|x| x.id).collect(),
        labels: s.iter().map(|x| x.label.clone()).collect(),
        hide_icon: s.iter().map(|x| x.os != 0 && os::tuning(x.os).map_or(true, |t| t.item == 0)).collect(),
        pools: s.iter().map(|x| x.pool.clone()).collect(),
        title: if s.len() < 2 { "PIÈCE OBTENUE".to_string() } else { k += 1; if n > 1 { format!("CHOISISSEZ UNE PIÈCE ({}/{})", k, n) } else { "CHOISISSEZ UNE PIÈCE".to_string() } },
    }).collect();
    let handles: Vec<u32> = sets.iter().flatten().map(|x| x.id).collect();
    for id in ids {
        if !handles.contains(id) { pages.push(rewardmenu::Page::part(*id, "PIÈCE OBTENUE")); }
    }
    Some(pages)
}

/// The game is about to commit the pending list (mgr = its item manager): take the parts not picked out of it, and the
/// carrier items of OS Tuning offers; a picked OS Tuning gains a level.
pub unsafe fn prune(mgr: usize) {
    let sets = match ARMED.lock() { Ok(mut a) => core::mem::take(&mut *a), Err(_) => return };
    if sets.is_empty() { return; }
    let gd = mgr - o::GD_ITEM_MGR;
    let mut drop = Vec::new();
    let mut kept = Vec::new();
    for (s, set) in sets.iter().enumerate() {
        let pick = rewardmenu::picked(s).min(set.len() - 1);
        for (j, x) in set.iter().enumerate() {
            let picked = j == pick;
            if picked && x.os != 0 { os::level_up(gd, x.os); }
            if picked && (x.os == 0 || x.keep) { kept.push(x.id); }
            else if x.handle != 0 { drop.push(x.handle); }
            else if x.os == 0 || x.keep { logf!("WARNING: reward choice: no handle for {:#X}, it stays granted", x.id); }
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
