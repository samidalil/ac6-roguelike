//! Reward choice (pick 1 of K): each of a mission's rewards (parts_per_mission) offers `reward_choices` parts. Nothing
//! is rolled at sortie (a failed sortie uses no roll): one placeholder item queued in the game's pending obtained-items
//! list makes the results open their parts step, where the game's "Pièces obtenues" opens our grid instead
//! (rewardmenu.rs, one page per reward). The first page is rolled right before the grid opens, each later page when
//! the page before it is confirmed (the earlier picks counted); just before the game commits the pending list the
//! placeholder is taken out of it and the picks put in. The offers are drawn by rarity from the reward candidates
//! (lib.rs `candidates`) with their own random generator, saved in event flags.

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
/// 0xRRGGBB), None when its pool shows none. `queued`: in the pending list since the sortie (first page); a later page's
/// pick is given at the commit.
#[derive(Clone, Debug)]
pub struct Offer { pub id: u32, pub handle: u32, pub os: u16, pub keep: bool, pub label: Option<(String, String)>, pub pool: Option<(String, u32)>, pub queued: bool }

/// The current sortie's offers, by reward (= grid page): the first page from the sortie, each later one once rolled.
static ARMED: std::sync::Mutex<Vec<Vec<Offer>>> = std::sync::Mutex::new(Vec::new());

/// The later pages of the sortie's rewards: rolled one by one at the results screen, each when the page before it is
/// confirmed, among `cands` (the reward candidates at the sortie) with the conditions checked again then.
struct Later { cands: Vec<Part>, pages: usize, k: usize, forced: Vec<Part> }
static LATER: std::sync::Mutex<Option<Later>> = std::sync::Mutex::new(None);
/// Sortie: `pages` rewards of `k` parts each, all rolled at the results.
pub fn arm_later(cands: Vec<Part>, pages: usize, k: usize) {
    if let Ok(mut l) = LATER.lock() { *l = if pages > 0 { Some(Later { cands, pages, k, forced: Vec::new() }) } else { None }; }
}
pub fn later_armed() -> bool { LATER.lock().is_ok_and(|l| l.is_some()) }
fn later_pages() -> usize { LATER.lock().ok().and_then(|l| l.as_ref().map(|l| l.pages)).unwrap_or(0) }
/// DEBUG `force_reward` / `force_os`: the first offer of page k is the k-th of these parts (owned or not).
pub fn force(parts: Vec<Part>) { if let Ok(mut l) = LATER.lock() { if let Some(l) = l.as_mut() { l.forced = parts; } } }
/// Pending handle of the sortie's placeholder item (0 = none), taken out at the commit.
static PLACEHOLDER: AtomicU32 = AtomicU32::new(0);
pub fn set_placeholder(h: u32) { PLACEHOLDER.store(h, Relaxed); }

fn page_of(s: &[Offer], title: String) -> rewardmenu::Page {
    rewardmenu::Page {
        ids: s.iter().map(|x| x.id).collect(),
        labels: s.iter().map(|x| x.label.clone()).collect(),
        hide_icon: s.iter().map(|x| x.os != 0 && os::tuning(x.os).map_or(true, |t| t.item == 0)).collect(),
        pools: s.iter().map(|x| x.pool.clone()).collect(),
        title,
    }
}
fn choice_title(k: usize, n: usize) -> String { if n > 1 { format!("CHOISISSEZ UNE PIÈCE ({}/{})", k, n) } else { "CHOISISSEZ UNE PIÈCE".to_string() } }

/// Results screen, game thread: roll reward page `p` (0-based) if it is the next page not rolled yet, the earlier
/// pages' picks counted (not committed yet): an OS Tuning at its max level with them, a unique part owned or picked, a
/// part offered its [max_occurrences] times (the earlier pages' offers included), a part of the shop rolled at the
/// clear: left out. The same part can come back on a later page. Its offers are counted for [max_occurrences] now.
pub unsafe fn roll_page(p: usize) -> Option<rewardmenu::Page> {
    let g = Game::new(mem::module_base());
    let (gd, man) = (g.game_data_fast()?, g.flag_man_fast()?);
    let later = LATER.lock().ok()?;
    let l = later.as_ref()?;
    let mut armed = ARMED.lock().ok()?;
    if p != armed.len() || p >= l.pages { return None; }
    let picks: Vec<Offer> = armed.iter().enumerate().filter_map(|(s, set)| set.get(rewardmenu::picked(s).min(set.len().saturating_sub(1))).cloned()).collect();
    let picked_levels = |row: u16| picks.iter().filter(|o| o.os == row).count() as i32;
    let lv = mem::rd_usize(gd + o::GD_OS_TUNING);
    let level = |row: u16| os::level_in(&g, man, lv, row).unwrap_or(0);
    let owned = g.owned_ids(gd);
    let counts = occur::read(&g, man);
    let shop = g.read_state(man).unwrap_or_default();
    // why a sortie candidate is out now (None = still in)
    let out = |c: &Part| -> Option<&'static str> {
        if c.is_os() && !os::tuning(c.os).is_some_and(|t| level(c.os) + picked_levels(c.os) < t.max) { return Some("max level"); }
        if !c.is_os() && c.unique() && (owned.contains(&c.grant_id()) || picks.iter().any(|o| o.id == c.grant_id())) { return Some("owned"); }
        if !c.is_os() && shop.contains(&c.idx) { return Some("in the shop"); }
        if c.max > 0 && occur::count(&counts, c) >= c.max as u32 { return Some("max occurrences"); }
        None
    };
    let (cands, gone): (Vec<(Part, Option<&str>)>, Vec<(Part, Option<&str>)>) = l.cands.iter().map(|c| (*c, out(c))).partition(|(_, why)| why.is_none());
    let cands: Vec<Part> = cands.into_iter().map(|(c, _)| c).collect();
    let left_out: Vec<String> = gone.iter().map(|(c, why)| format!("{} ({})", c.name(), why.unwrap_or(""))).collect();
    if !left_out.is_empty() { logf!("reward page {}: left out since the sortie: {}", p + 1, left_out.join(", ")); }
    let mut set = roll(&g, man, &cands, 1, l.k).pop().unwrap_or_default();
    if let Some(f) = l.forced.get(p) { set.retain(|c| c.idx != f.idx); set.insert(0, *f); set.truncate(l.k); }
    let offers: Vec<Offer> = set.iter().map(|c| {
        let keep = c.is_os() && c.equip_id != 0 && !owned.contains(&c.equip_id) && !picks.iter().any(|o| o.keep && o.id == c.equip_id);
        let id = if c.is_os() && !keep { o::OS_CARRIER_ITEM } else { c.grant_id() };
        let label = if c.is_os() { Some(os::label_at(c.os, level(c.os) + picked_levels(c.os) + 1)) } else { None };
        Offer { id, handle: 0, os: c.os, keep, label, pool: c.pool_label(), queued: false }
    }).collect();
    let counted: Vec<String> = set.iter().filter(|c| c.max > 0).map(|c| { let n = occur::record(&g, man, c); format!("{} {}/{}", c.name(), n, c.max) }).collect();
    logf!("reward page {}/{} rolled among {} candidate(s): {:?}{}", p + 1, l.pages, cands.len(), set.iter().map(|c| c.name()).collect::<Vec<_>>(),
        if counted.is_empty() { String::new() } else { format!("; max occurrences, offered: {}", counted.join(", ")) });
    let page = page_of(&offers, choice_title(p + 1, l.pages));
    armed.push(offers);
    Some(page)
}

/// The offers are queued (sortie, or re-queued after a restart).
pub fn arm(sets: Vec<Vec<Offer>>) {
    logf!("reward choice armed: {:X?}", sets.iter().map(|s| s.iter().map(|x| (x.id, x.handle, x.os)).collect::<Vec<_>>()).collect::<Vec<_>>());
    if let Ok(mut a) = ARMED.lock() { *a = sets; }
}
pub fn disarm() { if let Ok(mut a) = ARMED.lock() { a.clear(); } if let Ok(mut l) = LATER.lock() { *l = None; } PLACEHOLDER.store(0, Relaxed); }

/// Results screen: the pages of our grid for the game's pending parts list `ids` (None = no choice armed: the vanilla
/// dialog). One page per offer (a later reward's page empty until rolled), then one per other part of the list (a
/// mission's own reward; not the placeholder).
pub fn pages(ids: &[u32]) -> Option<Vec<rewardmenu::Page>> {
    let sets = ARMED.lock().ok()?.clone();
    if sets.is_empty() { return None; }
    let later = later_pages().saturating_sub(sets.len());
    let n = sets.iter().filter(|s| s.len() > 1).count() + later;
    let mut k = 0;
    let mut pages: Vec<rewardmenu::Page> = sets.iter().map(|s| page_of(s, if s.len() < 2 { "PIÈCE OBTENUE".to_string() } else { k += 1; choice_title(k, n) })).collect();
    for _ in 0..later { pages.push(rewardmenu::Page::parts(Vec::new(), String::new())); }
    let handles: Vec<u32> = sets.iter().flatten().map(|x| x.id).collect();
    let mut placeholder = PLACEHOLDER.load(Relaxed) != 0;
    // goods (0x4.. ids: combat logs, mission pickups) get no page from the game's class: leaving them out keeps our
    // pages in step with the class's
    for id in ids {
        if placeholder && *id == o::OS_CARRIER_ITEM { placeholder = false; continue; }
        if !handles.contains(id) && id >> 28 != 4 { pages.push(rewardmenu::Page::part(*id, "PIÈCE OBTENUE")); }
    }
    Some(pages)
}

/// The game is about to commit the pending list (mgr = its item manager): take the parts not picked out of it, and the
/// carrier items of OS Tuning offers; a picked OS Tuning gains a level.
pub unsafe fn prune(mgr: usize) {
    let sets = match ARMED.lock() { Ok(mut a) => core::mem::take(&mut *a), Err(_) => return };
    let placeholder = PLACEHOLDER.swap(0, Relaxed);
    if sets.is_empty() && placeholder == 0 { return; }
    let gd = mgr - o::GD_ITEM_MGR;
    let mut drop = Vec::new();
    if placeholder != 0 { drop.push(placeholder); }
    let mut kept = Vec::new();
    for (s, set) in sets.iter().enumerate() {
        let pick = rewardmenu::picked(s).min(set.len().saturating_sub(1));
        for (j, x) in set.iter().enumerate() {
            let picked = j == pick;
            if picked && x.os != 0 { os::level_up(gd, x.os); }
            if picked { event::emit(event::Event::Pick); }
            if picked && (x.os == 0 || x.keep) { kept.push(x.id); }
            else if x.handle != 0 { drop.push(x.handle); }
            else if x.queued && (x.os == 0 || x.keep) { logf!("WARNING: reward choice: no handle for {:#X}, it stays granted", x.id); }
        }
    }
    let (b, e) = (mem::rd_usize(mgr + o::MGR_PENDING_BEGIN), mem::rd_usize(mgr + o::MGR_PENDING_END));
    if b == 0 || e < b || e - b > 0x1000 { log("WARNING: reward choice: pending list unreadable at the commit"); return; }
    let all: Vec<u32> = (0..(e - b) / 4).map(|i| mem::rd_u32(b + 4 * i)).collect();
    let keep: Vec<u32> = all.iter().copied().filter(|h| !drop.contains(h)).collect();
    for (i, h) in keep.iter().enumerate() { mem::wr_u32(b + 4 * i, *h); }
    core::ptr::write_unaligned((mgr + o::MGR_PENDING_END) as *mut usize, b + 4 * keep.len());
    // the picks were never queued: into the list now, the commit inserts them with the rest
    let g = Game::new(mem::module_base());
    for x in sets.iter().enumerate().filter_map(|(s, set)| set.get(rewardmenu::picked(s).min(set.len().saturating_sub(1)))).filter(|x| !x.queued && (x.os == 0 || x.keep)) {
        let (r, err) = g.give_deferred(gd, x.id);
        logf!("reward choice: pick {:#X} queued at the commit -> rax={} err={}", x.id, r, err);
    }
    logf!("reward choice: picked {:X?}; pending {:X?} -> {:X?} at the commit", kept, all, g.pending(gd));
}
