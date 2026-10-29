//! OS Tuning as mission rewards (`[rewards] os_rewards`). Every OS Tuning (17 OsReinforce rows) can be offered until
//! its level reaches the max (levels with an OST chip cost); a pick adds one level (i32 per tuning at
//! [PlayerGameData+0x1368], RE_NOTES "OS Tuning rewards"). The four core expansions also need their item:
//! Assemblage's EXTENSION list shows one only with the item owned AND a level above 0, so an expansion not owned yet
//! queues its item, kept when picked. Every other OS offer queues a carrier item (the results' parts dialog only
//! opens on a non-empty pending list), always taken out of the list at the commit (choice.rs). The vanilla OS Tuning
//! menu stays locked (UnlockParam_MenuCommand row pointed at a flag nothing sets) and its unlock notification no
//! longer lists it. [os_tuning] rewrites a tuning's effect value per level (and so its max level) in OsReinforce: the
//! game reads these values at each hit (RE_NOTES "OS Tuning values"). Our own chips (chip.rs) are tunings too, rows
//! 9000 + id, with their level in event flags instead of the game's array.

use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub struct Tuning { pub row: u16, off: usize, name: u32, pub item: u32, pub max: i32, pub chip: bool }

static TUNINGS: OnceLock<Vec<Tuning>> = OnceLock::new();
pub fn tunings() -> &'static [Tuning] { TUNINGS.get().map_or(&[], |v| v.as_slice()) }
pub fn tuning(row: u16) -> Option<Tuning> { tunings().iter().find(|t| t.row == row).copied() }

/// Boot: [os_tuning] values written into the live OsReinforce param, then the max level of each tuning from it, then
/// the chips (max = their number of levels). False while the param is not there yet.
pub unsafe fn init(base: usize) -> bool {
    if TUNINGS.get().is_some() { return true; }
    let Some(f) = param_index(base, "OsReinforce").and_then(|i| param_file(base, i)) else { return false };
    let n = apply_values(base);
    if n > 0 { logf!("OS Tuning: [os_tuning] wrote {} field(s)", n); }
    let rows = param_rows(f);
    let mut v = Vec::new();
    for &(row, off, name, item) in &o::OS_TUNINGS {
        let Some(&(_, r)) = rows.iter().find(|x| x.0 == row as u32) else { logf!("OS Tuning: OsReinforce row {} missing", row); continue };
        let max = (0..o::OSR_LEVELS).filter(|k| mem::rd_i32(r + o::OSR_LEVEL0_COST + k * o::OSR_LEVEL_STRIDE) > 0).count() as i32;
        if max > 0 { v.push(Tuning { row, off, name, item, max, chip: false }); }
    }
    for c in chip::chips() { v.push(Tuning { row: c.row(), off: 0, name: 0, item: 0, max: c.values.len() as i32, chip: true }); }
    logf!("OS Tuning: {} tunings, max levels {:?}", v.len(), v.iter().map(|t| (t.row, t.max)).collect::<Vec<_>>());
    let _ = TUNINGS.set(v);
    true
}

/// OsReinforce rows as the game shipped them, by row id: (max level byte, [(value, cost, text id); 5]). Taken the first
/// time a row is seen (boot, before any write).
type Levels = (u8, [(i32, i32, i32); o::OSR_LEVELS]);
static VANILLA: std::sync::Mutex<Vec<(u32, Levels)>> = std::sync::Mutex::new(Vec::new());

unsafe fn read_levels(r: usize) -> Levels {
    let mut l = [(0, 0, 0); o::OSR_LEVELS];
    for (k, e) in l.iter_mut().enumerate() {
        let a = r + k * o::OSR_LEVEL_STRIDE;
        *e = (mem::rd_i32(a + o::OSR_LEVEL0_VALUE), mem::rd_i32(a + o::OSR_LEVEL0_COST), mem::rd_i32(a + o::OSR_LEVEL0_TEXT));
    }
    (mem::rd_u8(r + o::OSR_MAX_LEVEL), l)
}

/// The OsReinforce row for `values` (one per level): those levels exist (chip cost > 0, the game's cost or the last
/// one), the levels above repeat the last value with cost 0 (a save already above the new max keeps the max effect),
/// max level byte = their count. Text: the game's text of the level with the same value when there is one (repair
/// kits: one text per value), else the level's own.
fn levels_for(van: &Levels, values: &[i32]) -> Levels {
    let n = values.len().min(o::OSR_LEVELS);
    let mut l = van.1;
    let mut cost = 1;
    for k in 0..o::OSR_LEVELS {
        let v = values[k.min(n - 1)];
        if van.1[k].1 > 0 { cost = van.1[k].1; }
        let text = van.1.iter().find(|e| e.0 == v && e.2 != 0).map_or(van.1[k.min(n - 1)].2, |e| e.2);
        l[k] = (v, if k < n { cost } else { 0 }, text);
    }
    (n as u8, l)
}

/// [os_tuning]: write the configured values into the live OsReinforce rows (ACS also into its sub-rows), once at boot
/// from `init` (the params are loaded once per game start). Returns the fields changed.
pub unsafe fn apply_values(base: usize) -> usize {
    let cfg = config::get();
    if cfg.os_tuning.is_empty() { return 0; }
    let Some(f) = param_index(base, "OsReinforce").and_then(|i| param_file(base, i)) else { return 0 };
    let rows = param_rows(f);
    let mut van = VANILLA.lock().unwrap_or_else(|e| e.into_inner());
    for &(id, r) in &rows { if !van.iter().any(|x| x.0 == id) { van.push((id, read_levels(r))); } }
    let mut changed = 0;
    for (row, values) in &cfg.os_tuning {
        let targets: Vec<u16> = if *row == o::OSR_ACS { [o::OSR_ACS].iter().chain(o::OSR_ACS_SUBROWS.iter()).copied().collect() } else { vec![*row] };
        for t in targets {
            let (Some(&(_, r)), Some((_, v))) = (rows.iter().find(|x| x.0 == t as u32), van.iter().find(|x| x.0 == t as u32)) else { continue };
            let want = levels_for(v, values);
            let cur = read_levels(r);
            if cur.0 != want.0 { mem::wr_u8(r + o::OSR_MAX_LEVEL, want.0); changed += 1; }
            for k in 0..o::OSR_LEVELS {
                let a = r + k * o::OSR_LEVEL_STRIDE;
                for (off, c, w) in [(o::OSR_LEVEL0_VALUE, cur.1[k].0, want.1[k].0), (o::OSR_LEVEL0_COST, cur.1[k].1, want.1[k].1), (o::OSR_LEVEL0_TEXT, cur.1[k].2, want.1[k].2)] {
                    if c != w { mem::wr_i32(a + off, w); changed += 1; }
                }
            }
        }
    }
    changed
}

unsafe fn levels(gd: usize) -> Option<usize> {
    let p = mem::rd_usize(gd + o::GD_OS_TUNING);
    if p == 0 || !mem::is_readable(p, 0x50) { return None; }
    Some(p)
}

/// Current level of a tuning (None: unknown row, or the tuning data is not loaded).
pub unsafe fn level(gd: usize, row: u16) -> Option<i32> {
    let t = tuning(row)?;
    if t.chip { chip::level(row) } else { Some(mem::rd_i32(levels(gd)? + t.off)) }
}

/// A chip's new level (0..max) and its effect; returns (old, new).
unsafe fn set_chip(row: u16, f: impl Fn(i32) -> i32, why: &str) -> Option<(i32, i32)> {
    let old = chip::level(row)?;
    let new = f(old).clamp(0, tuning(row)?.max);
    if !chip::set_level(row, new) { logf!("WARNING: chip {}: level not saved", row); return None; }
    logf!("chip {} ({}): level {} -> {}/{} ({})", row, name(row), old, new, tuning(row)?.max, why);
    Some((old, new))
}

/// One more level (commit, game thread), capped at the max.
pub unsafe fn level_up(gd: usize, row: u16) {
    if tuning(row).is_some_and(|t| t.chip) { set_chip(row, |l| l + 1, "at the commit"); return; }
    let (Some(lv), Some(t)) = (levels(gd), tuning(row)) else { logf!("WARNING: OS Tuning {}: levels unreadable at the commit", row); return };
    let old = mem::rd_i32(lv + t.off);
    let new = (old + 1).min(t.max).max(old);
    mem::wr_i32(lv + t.off, new);
    logf!("OS Tuning {} ({}): level {} -> {}/{} at the commit", row, name(row), old, new, t.max);
}

/// Failure loss of a non-expansion tuning ([lost_on_failure] = 1): one level less. Returns (old, new).
pub unsafe fn level_down(gd: usize, row: u16) -> Option<(i32, i32)> {
    if tuning(row).is_some_and(|t| t.chip) { return set_chip(row, |l| l - 1, "lost"); }
    let (Some(lv), Some(t)) = (levels(gd), tuning(row)) else { logf!("WARNING: OS Tuning {}: levels unreadable, no level lost", row); return None };
    let old = mem::rd_i32(lv + t.off);
    let new = (old - 1).max(0);
    mem::wr_i32(lv + t.off, new);
    logf!("OS Tuning {} ({}): level {} -> {}/{} (lost)", row, name(row), old, new, t.max);
    Some((old, new))
}

/// Failure loss of a core expansion: level back to 0 (Assemblage stops listing it).
pub unsafe fn reset(gd: usize, row: u16) {
    let (Some(lv), Some(t)) = (levels(gd), tuning(row)) else { logf!("WARNING: OS Tuning {}: levels unreadable, not reset", row); return };
    logf!("OS Tuning {} ({}): level {} -> 0 (lost)", row, name(row), mem::rd_i32(lv + t.off));
    mem::wr_i32(lv + t.off, 0);
}

/// [max_occurrences]: the tuning was offered its max number of times this run (save bit OS_GONE + its rank in
/// OS_TUNINGS, chips CHIP_GONE; occur.rs keeps the counts below the max). Rank: OS_TUNINGS index, then chips after
/// them by id (occur.rs keys).
pub fn rank(row: u16) -> Option<u32> {
    o::OS_TUNINGS.iter().position(|t| t.0 == row).map(|k| k as u32)
        .or_else(|| chip::chip(row).map(|c| o::OS_TUNINGS.len() as u32 + c.id as u32 - 1))
}
pub unsafe fn gone(g: &Game, man: usize, row: u16) -> bool {
    if chip::chip(row).is_some() { return chip::gone(g, man, row); }
    o::OS_TUNINGS.iter().position(|t| t.0 == row).is_some_and(|k| g.get_flag(man, o::OS_GONE + k as u32) == Some(true))
}
pub unsafe fn set_gone(g: &Game, man: usize, row: u16) {
    if chip::chip(row).is_some() { return chip::set_gone(g, man, row); }
    if let Some(k) = o::OS_TUNINGS.iter().position(|t| t.0 == row) { g.put_flag(man, o::OS_GONE + k as u32, true); }
}
/// The core expansion tuning of an item id (tag bits ignored).
pub fn of_item(item: u32) -> Option<Tuning> { tunings().iter().find(|t| t.item != 0 && t.item == item & 0x0FFF_FFFF).copied() }

pub fn name(row: u16) -> String {
    if let Some(c) = chip::chip(row) { return c.name.clone(); }
    tuning(row).and_then(|t| unsafe { msg::text(o::MSG_CAT_OS_NAME, t.name) }).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("OS {}", row))
}

/// Grid tile texts of an offer: (name, "AMÉLIORATION OS  LV n/max") with n the level it brings; a chip's name also
/// gives that level's effect ("Blindage renforcé : PB +5 %").
pub unsafe fn label(gd: usize, row: u16) -> (String, String) {
    let t = tuning(row);
    let next = level(gd, row).unwrap_or(0) + 1;
    let menu = msg::text(o::MSG_CAT_MENU_TITLE, o::MSG_OS_MENU).unwrap_or_default();
    let name = match chip::chip(row) {
        Some(c) => format!("{} : {} {}{} %", c.name, c.stat, if c.sign() < 0 { "-" } else { "+" }, chip::value(row, next)),
        None => name(row),
    };
    (name, format!("{}  LV {}/{}", menu, next, t.map_or(next, |t| t.max)))
}

/// Keep the vanilla OS Tuning menu locked: its UnlockParam_MenuCommand row checks a flag nothing sets. Applied once at
/// boot (the params are loaded once per game start). Returns true when it changed the row.
pub unsafe fn lock_menu(base: usize) -> bool {
    let Some(f) = param_index(base, "UnlockParam_MenuCommand").and_then(|i| param_file(base, i)) else { return false };
    let Some((_, r)) = param_rows(f).into_iter().find(|x| x.0 == o::MENU_ROW_OS_TUNING) else { return false };
    let flag = r + o::UNLOCK_ROW_FLAG;
    if mem::rd_u32(flag) != o::FLAG_OS_TUNING_MENU { return false; }
    mem::wr_u32(flag, o::FLAG_NEVER);
    true
}

/// Silence the OS Tuning lines (2 and 3) of ALLMIND's message granting OS Tuning + Arena: their TalkParam rows get
/// msgId and voiceId -1, and the player goes from line 1 straight to line 4 (one voiced row per line). The rows stay:
/// the message's line count is not taken from them (removing rows made it play the next message's lines). Applied once
/// at boot like `lock_menu`; a row 913001011 still showing the OS line means an untouched param. True when it changed
/// the rows.
pub unsafe fn cut_allmind_talk(base: usize) -> bool {
    let Some(f) = param_index(base, "TalkParam").and_then(|i| param_file(base, i)) else { return false };
    let rows = param_rows(f);
    let Some(&(_, r)) = rows.iter().find(|x| x.0 == o::TALK_ALLMIND_OS) else { return false };
    if mem::rd_u32(r + o::TALK_ROW_MSG) != o::TALK_ALLMIND_OS_MSG { return false; }
    for &(_, r) in rows.iter().filter(|x| (o::TALK_ALLMIND_OS..o::TALK_ALLMIND_OS + o::TALK_ALLMIND_CUT).contains(&x.0)) {
        mem::wr_i32(r + o::TALK_ROW_MSG, -1);
        mem::wr_i32(r + o::TALK_ROW_VOICE, -1);
    }
    true
}
