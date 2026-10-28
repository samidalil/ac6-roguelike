//! OS Tuning as mission rewards (`[rewards] os_rewards`). Every OS Tuning (17 OsReinforce rows) can be offered until
//! its level reaches the max (levels with an OST chip cost); a pick adds one level (i32 per tuning at
//! [PlayerGameData+0x1368], RE_NOTES "OS Tuning rewards"). The four core expansions also need their item:
//! Assemblage's EXTENSION list shows one only with the item owned AND a level above 0, so an expansion not owned yet
//! queues its item, kept when picked. Every other OS offer queues a carrier item (the results' parts dialog only
//! opens on a non-empty pending list), always taken out of the list at the commit (choice.rs). The vanilla OS Tuning
//! menu stays locked (UnlockParam_MenuCommand row pointed at a flag nothing sets) and its unlock notification no
//! longer lists it.

use super::*;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug)]
pub struct Tuning { pub row: u16, off: usize, name: u32, pub item: u32, pub max: i32 }

static TUNINGS: OnceLock<Vec<Tuning>> = OnceLock::new();
pub fn tunings() -> &'static [Tuning] { TUNINGS.get().map_or(&[], |v| v.as_slice()) }
pub fn tuning(row: u16) -> Option<Tuning> { tunings().iter().find(|t| t.row == row).copied() }

/// Boot: max level of each tuning from the live OsReinforce param. False while the param is not there yet.
pub unsafe fn init(base: usize) -> bool {
    if TUNINGS.get().is_some() { return true; }
    let Some(f) = param_index(base, "OsReinforce").and_then(|i| param_file(base, i)) else { return false };
    let rows = param_rows(f);
    let mut v = Vec::new();
    for &(row, off, name, item) in &o::OS_TUNINGS {
        let Some(&(_, r)) = rows.iter().find(|x| x.0 == row as u32) else { logf!("OS Tuning: OsReinforce row {} missing", row); continue };
        let max = (0..5).filter(|k| mem::rd_i32(r + o::OSR_LEVEL0_COST + k * o::OSR_LEVEL_STRIDE) > 0).count() as i32;
        if max > 0 { v.push(Tuning { row, off, name, item, max }); }
    }
    logf!("OS Tuning: {} tunings, max levels {:?}", v.len(), v.iter().map(|t| (t.row, t.max)).collect::<Vec<_>>());
    let _ = TUNINGS.set(v);
    true
}

unsafe fn levels(gd: usize) -> Option<usize> {
    let p = mem::rd_usize(gd + o::GD_OS_TUNING);
    if p == 0 || !mem::is_readable(p, 0x50) { return None; }
    Some(p)
}

/// Current level of a tuning (None: unknown row, or the tuning data is not loaded).
pub unsafe fn level(gd: usize, row: u16) -> Option<i32> { Some(mem::rd_i32(levels(gd)? + tuning(row)?.off)) }

/// One more level (commit, game thread), capped at the max.
pub unsafe fn level_up(gd: usize, row: u16) {
    let (Some(lv), Some(t)) = (levels(gd), tuning(row)) else { logf!("WARNING: OS Tuning {}: levels unreadable at the commit", row); return };
    let old = mem::rd_i32(lv + t.off);
    let new = (old + 1).min(t.max).max(old);
    mem::wr_i32(lv + t.off, new);
    logf!("OS Tuning {} ({}): level {} -> {}/{} at the commit", row, name(row), old, new, t.max);
}

/// Failure loss of a core expansion: level back to 0 (Assemblage stops listing it).
pub unsafe fn reset(gd: usize, row: u16) {
    let (Some(lv), Some(t)) = (levels(gd), tuning(row)) else { logf!("WARNING: OS Tuning {}: levels unreadable, not reset", row); return };
    logf!("OS Tuning {} ({}): level {} -> 0 (lost)", row, name(row), mem::rd_i32(lv + t.off));
    mem::wr_i32(lv + t.off, 0);
}

/// [unique_occurrences]: the tuning was offered once this run (save bit OS_GONE + its rank in OS_TUNINGS).
fn rank(row: u16) -> Option<u32> { o::OS_TUNINGS.iter().position(|t| t.0 == row).map(|k| k as u32) }
pub unsafe fn gone(g: &Game, man: usize, row: u16) -> bool { rank(row).is_some_and(|k| g.get_flag(man, o::OS_GONE + k) == Some(true)) }
pub unsafe fn set_gone(g: &Game, man: usize, row: u16) { if let Some(k) = rank(row) { g.put_flag(man, o::OS_GONE + k, true); } }
/// The core expansion tuning of an item id (tag bits ignored).
pub fn of_item(item: u32) -> Option<Tuning> { tunings().iter().find(|t| t.item != 0 && t.item == item & 0x0FFF_FFFF).copied() }

pub fn name(row: u16) -> String {
    tuning(row).and_then(|t| unsafe { msg::text(o::MSG_CAT_OS_NAME, t.name) }).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("OS {}", row))
}

/// Grid tile texts of an offer: (name, "AMÉLIORATION OS  LV n/max") with n the level it brings.
pub unsafe fn label(gd: usize, row: u16) -> (String, String) {
    let t = tuning(row);
    let next = level(gd, row).unwrap_or(0) + 1;
    let menu = msg::text(o::MSG_CAT_MENU_TITLE, o::MSG_OS_MENU).unwrap_or_default();
    (name(row), format!("{}  LV {}/{}", menu, next, t.map_or(next, |t| t.max)))
}

/// Keep the vanilla OS Tuning menu locked: its UnlockParam_MenuCommand row checks a flag nothing sets. Idempotent
/// (re-applied with the economy, the params can be reloaded). Returns true when it changed the row.
pub unsafe fn lock_menu(base: usize) -> bool {
    let Some(f) = param_index(base, "UnlockParam_MenuCommand").and_then(|i| param_file(base, i)) else { return false };
    let Some((_, r)) = param_rows(f).into_iter().find(|x| x.0 == o::MENU_ROW_OS_TUNING) else { return false };
    let flag = r + o::UNLOCK_ROW_FLAG;
    if mem::rd_u32(flag) != o::FLAG_OS_TUNING_MENU { return false; }
    mem::wr_u32(flag, o::FLAG_NEVER);
    true
}

/// Silence the OS Tuning lines (2 and 3) of ALLMIND's message after Wallclimber: their TalkParam rows get msgId and
/// voiceId -1, and the player goes from line 1 straight to line 4 (one voiced row per line). The rows stay: the
/// message's line count is not taken from them (removing rows made it play the next message's lines). Idempotent like
/// `lock_menu`: a row 913001011 still showing the OS line means a fresh param. True when it changed the rows.
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
