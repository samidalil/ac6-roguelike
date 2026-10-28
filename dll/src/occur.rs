//! `[max_occurrences]`: an item listed with N is offered at most N times per run (reward grid or shop, picked or
//! not). Once at its max it gets its "never again" bit (ROWS_LOST for a lineup row, OS_GONE for an OS Tuning); until
//! then its offer count lives in a save-backed slot table (offsets OCCUR_*), one slot per item offered and not used up.

use super::*;

const SLOT_BITS: u32 = o::OCCUR_KEY_BITS + o::OCCUR_COUNT_BITS;
fn slot(k: u32) -> u32 { o::OCCUR_BASE + k * SLOT_BITS }

/// Slot key of a part: lineup row + 1, or after the rows for an OS Tuning (its rank in OS_TUNINGS).
fn key(p: &Part) -> Option<u32> {
    if p.is_os() { os::rank(p.os).map(|r| o::ROW_COUNT as u32 + 1 + r) } else { Some(p.idx as u32 + 1) }
}

/// (key, count) of every slot in use.
pub unsafe fn read(g: &Game, man: usize) -> Vec<(u32, u32)> {
    (0..o::OCCUR_SLOTS).filter_map(|k| {
        let key = g.get_bits(man, slot(k), o::OCCUR_KEY_BITS);
        (key != 0).then(|| (key, g.get_bits(man, slot(k) + o::OCCUR_KEY_BITS, o::OCCUR_COUNT_BITS)))
    }).collect()
}

/// Offers so far of a part not used up yet (0 when it has no slot).
pub fn count(counts: &[(u32, u32)], p: &Part) -> u32 {
    key(p).and_then(|k| counts.iter().find(|c| c.0 == k)).map_or(0, |c| c.1)
}

/// One more offer of a listed part; returns its count. At the max: the "never again" bit, slot freed. No free slot
/// for a new item: used up at once (warning).
pub unsafe fn record(g: &Game, man: usize, p: &Part) -> u32 {
    let Some(key) = key(p) else { return 0 };
    let slots: Vec<u32> = (0..o::OCCUR_SLOTS).map(|k| g.get_bits(man, slot(k), o::OCCUR_KEY_BITS)).collect();
    let mine = slots.iter().position(|&s| s == key);
    let n = mine.map_or(0, |k| g.get_bits(man, slot(k as u32) + o::OCCUR_KEY_BITS, o::OCCUR_COUNT_BITS)) + 1;
    let free = mine.or_else(|| slots.iter().position(|&s| s == 0));
    let used_up = |n: u32| {
        if p.is_os() { os::set_gone(g, man, p.os) } else { g.put_row(man, o::ROWS_LOST, p.idx, true) }
        n
    };
    if n >= p.max as u32 {
        if let Some(k) = mine { g.put_bits(man, slot(k as u32), SLOT_BITS, 0); }
        return used_up(n);
    }
    let Some(k) = free else {
        logf!("WARNING: [max_occurrences]: all {} count slots in use, {} counts as used up", o::OCCUR_SLOTS, p.name());
        return used_up(p.max as u32);
    };
    g.put_bits(man, slot(k as u32), o::OCCUR_KEY_BITS, key);
    g.put_bits(man, slot(k as u32) + o::OCCUR_KEY_BITS, o::OCCUR_COUNT_BITS, n);
    n
}
