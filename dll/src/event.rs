//! Events and actions (step 2 of RE_NOTES "Modular chips: architecture and roadmap"). The engine emits an event at the
//! points the mod already detects (save load, sortie, clear, death, commit, failure, shop reroll, pick, sale...);
//! content subscribes with `[on.<event>]` sections in its files and `on_<event>` functions in its scripts (script.rs),
//! which run for each chip at level 1 or more. `flag` and `level` act at once (flag writes); `credits` and `give` are queued for the
//! game thread in the garage (penalty.rs frame hook), then listed in a game dialog.

use super::*;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    SaveLoaded, Title, GarageOpened, Deployment, MissionStart, Death, Respawn, MissionCleared, Results, Commit,
    MissionFailed, GameOver, ShopReroll, Pick, PartSold, AutoSell, EnemyDestroyed, CoreExpansion, EnemyStaggered,
}
pub const EVENTS: [(&str, Event); 19] = [
    ("save_loaded", Event::SaveLoaded), ("title", Event::Title), ("garage_opened", Event::GarageOpened),
    ("deployment", Event::Deployment), ("mission_start", Event::MissionStart), ("death", Event::Death),
    ("respawn", Event::Respawn), ("mission_cleared", Event::MissionCleared), ("results", Event::Results),
    ("commit", Event::Commit), ("mission_failed", Event::MissionFailed), ("game_over", Event::GameOver),
    ("shop_reroll", Event::ShopReroll), ("pick", Event::Pick), ("part_sold", Event::PartSold), ("auto_sell", Event::AutoSell),
    ("enemy_destroyed", Event::EnemyDestroyed), ("core_expansion", Event::CoreExpansion),
    ("enemy_staggered", Event::EnemyStaggered),
];
pub fn event(name: &str) -> Option<Event> { EVENTS.iter().find(|e| e.0.eq_ignore_ascii_case(name)).map(|e| e.1) }
pub fn name(e: Event) -> &'static str { EVENTS.iter().find(|x| x.1 == e).map_or("?", |x| x.0) }

/// An amount: fixed, one value per chip level, or the chip's own level value (`"value"`).
#[derive(Clone, Debug)]
pub enum Amount { Fixed(i32), PerLevel(Vec<i32>), Value }
impl Amount {
    pub fn parse(v: &config::Value) -> Option<Amount> {
        match v {
            config::Value::Num(n) => Some(Amount::Fixed(*n as i32)),
            config::Value::Text(t) if t.trim().eq_ignore_ascii_case("value") => Some(Amount::Value),
            config::Value::Text(t) => {
                let v: Vec<i32> = t.split('/').map(|x| x.trim().trim_start_matches('+').parse::<i32>().ok()).collect::<Option<_>>()?;
                match v.len() { 0 => None, 1 => Some(Amount::Fixed(v[0])), _ => Some(Amount::PerLevel(v)) }
            }
        }
    }
    fn at(&self, values: &[i32], lv: i32) -> i32 {
        let k = (lv.max(1) - 1) as usize;
        match self {
            Amount::Fixed(n) => *n,
            Amount::PerLevel(v) => v[k.min(v.len() - 1)],
            Amount::Value => values.get(k.min(values.len().saturating_sub(1))).copied().unwrap_or(0),
        }
    }
}

/// What an `[on.<event>]` line does.
#[derive(Clone, Debug)]
pub enum Action {
    /// `credits = N`: COAM added (or taken when negative), in the garage
    Credits(Amount),
    /// `give = "part name"` (or equipId): one copy granted in the garage (a booster / generator / FCS only if not owned)
    Give(String),
    /// `"level.<row or name>" = "+1"`: OS Tuning or chip level change
    Level(String, Amount),
    /// `"flag.<id>" = 1 / 0`: event flag set / cleared
    Flag(u32, bool),
    /// `speffect = <SpEffectParam row>` (or one row per level, "a/b/c"): applied to the player's AC on the game thread
    /// (from `mission_start`: to the mission's AC, not the garage's)
    SpEffect(Amount),
    /// `resupply = "a/b/c"`: every weapon of the player's AC gets this % of its total ammunition (the game's resupply,
    /// game thread; in a mission only)
    Resupply(Amount),
    /// `energy = "a/b/c"`: the player's AC gets this % of its EN capacity back (game thread; in a mission only)
    Energy(Amount),
}
impl Action {
    /// An `[on.<event>]` line: key and value.
    pub fn parse(key: &str, v: &config::Value) -> Result<Action, String> {
        let k = key.trim();
        if k.eq_ignore_ascii_case("credits") { return Amount::parse(v).map(Action::Credits).ok_or("credits = number, \"a/b/c\" or \"value\"".into()); }
        if k.eq_ignore_ascii_case("give") {
            return match v { config::Value::Text(t) => Ok(Action::Give(t.clone())), config::Value::Num(n) => Ok(Action::Give(n.to_string())) };
        }
        if k.eq_ignore_ascii_case("speffect") {
            return Amount::parse(v).filter(|a| !matches!(a, Amount::Value)).map(Action::SpEffect).ok_or("speffect = <SpEffectParam row id> or \"a/b/c\"".into());
        }
        if k.eq_ignore_ascii_case("resupply") {
            return Amount::parse(v).map(Action::Resupply).ok_or("resupply = % or \"a/b/c\"".into());
        }
        if k.eq_ignore_ascii_case("energy") {
            return Amount::parse(v).map(Action::Energy).ok_or("energy = % or \"a/b/c\"".into());
        }
        if let Some(r) = k.strip_prefix("level.") { return Amount::parse(v).map(|a| Action::Level(r.to_string(), a)).ok_or("level.<row> = \"+1\"".into()); }
        if let Some(f) = k.strip_prefix("flag.") {
            let id = f.parse::<u32>().map_err(|_| format!("flag.<id>: {} is not a flag id", f))?;
            return match v { config::Value::Num(n) => Ok(Action::Flag(id, *n != 0)), _ => Err("flag.<id> = 1 or 0".into()) };
        }
        Err(format!("unknown action {} (credits, give, speffect, resupply, energy, level.<row>, flag.<id>)", k))
    }
}

/// Garage work queued by `credits` / `give`: (COAM change, item names to give, dialog lines).
struct Garage { credits: i64, give: Vec<String>, lines: Vec<String> }
static GARAGE: Mutex<Garage> = Mutex::new(Garage { credits: 0, give: Vec::new(), lines: Vec::new() });
/// Dialog lines of applied actions, and the garage title count when they were queued: shown once the garage menu
/// is on screen again (after a mission the garage plays its notification and talk first).
static DIALOG: Mutex<(Vec<String>, u64)> = Mutex::new((Vec::new(), 0));

/// The engine reached `e`: run the actions of every chip at level 1+ subscribed to it, and queue its script's handler.
pub fn emit(e: Event) { emit_with(e, &[]) }
/// `emit` with fields for the scripts' event table (`e.<name>`).
pub fn emit_with(e: Event, data: &[(&'static str, f64)]) {
    let subs: Vec<&chip::Chip> = chip::chips().iter().filter(|c| c.on.iter().any(|h| h.0 == e) || c.script_on.contains(&e)).collect();
    if subs.is_empty() { return; }
    let base = mem::module_base();
    let g = unsafe { Game::new(base) };
    let Some(man) = (unsafe { g.flag_man() }) else { return };
    for c in subs {
        let lv = unsafe { chip::level(c.row()) }.unwrap_or(0);
        if lv == 0 { continue; }
        for (_, a) in c.on.iter().filter(|h| h.0 == e) {
            logf!("event {}: chip {} (LV {}) -> {:?}", name(e), c.name, lv, a);
            unsafe { run(&g, man, c, lv, a, e) };
        }
        if c.script_on.contains(&e) {
            let not = if e == Event::MissionStart { unsafe { player_chr(base) } } else { 0 };
            script::queue(c.id, e, lv, not, data);
        }
    }
}

/// A script's action (script.rs, game thread): `run` for the chip at `lv` on event `e`.
pub unsafe fn act(c: &chip::Chip, lv: i32, a: &Action, e: Event) {
    let g = Game::new(mem::module_base());
    let Some(man) = g.flag_man() else { return };
    run(&g, man, c, lv, a, e);
}

unsafe fn run(g: &Game, man: usize, c: &chip::Chip, lv: i32, a: &Action, e: Event) {
    match a {
        Action::Credits(n) => {
            let v = n.at(&c.values, lv);
            let mut q = GARAGE.lock().unwrap_or_else(|e| e.into_inner());
            q.credits += v as i64;
            q.lines.push(format!("{} : {}{} COAM", c.name, if v < 0 { "-" } else { "+" }, penalty::money(v.abs() as i64)));
        }
        Action::Give(name) => {
            let mut q = GARAGE.lock().unwrap_or_else(|e| e.into_inner());
            q.give.push(name.clone());
        }
        Action::Level(target, n) => {
            let Some(row) = row_of(target) else { logf!("event: {}: no OS Tuning or chip {}", c.name, target); return };
            let d = n.at(&c.values, lv);
            let Some(gd) = g.game_data_fast() else { return };
            for _ in 0..d.abs() { if d > 0 { os::level_up(gd, row); } else { os::level_down(gd, row); } }
        }
        Action::Flag(id, v) => { g.put_flag(man, *id, *v); }
        Action::Resupply(n) => queue_resupply(n.at(&c.values, lv)),
        Action::Energy(n) => queue_energy(n.at(&c.values, lv)),
        Action::SpEffect(n) => queue_speffect(n.at(&c.values, lv), e == Event::MissionStart),
    }
}

/// SpEffects waiting for the game thread: (id, a player character it must not go to). At the sortie the garage's own
/// player character still exists: a mission SpEffect waits for the mission's.
static SPEFFECTS: Mutex<Vec<(i32, usize)>> = Mutex::new(Vec::new());
pub fn queue_speffect(id: i32, next_character: bool) {
    push_speffect(id, if next_character { unsafe { player_chr(mem::module_base()) } } else { 0 });
}
/// A SpEffect for the player, not for the character `not` (0: any).
pub fn push_speffect(id: i32, not: usize) { SPEFFECTS.lock().unwrap_or_else(|e| e.into_inner()).push((id, not)); }
unsafe fn player_chr(base: usize) -> usize {
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    if wcm == 0 { 0 } else { mem::rd_usize(wcm + o::WCM_PLAYER) }
}

/// Resupplies waiting for the game thread, in % of each weapon's total.
static RESUPPLIES: Mutex<Vec<i32>> = Mutex::new(Vec::new());
pub fn queue_resupply(pct: i32) { if pct > 0 { RESUPPLIES.lock().unwrap_or_else(|e| e.into_inner()).push(pct); } }
/// Game thread, every frame: the queued resupplies on the player's weapons (dropped when there is no player AC).
pub unsafe fn apply_resupplies() {
    let q: Vec<i32> = core::mem::take(&mut *RESUPPLIES.lock().unwrap_or_else(|e| e.into_inner()));
    for pct in q { chip::resupply_player(pct as f32 / 100.0); }
}

/// EN refills waiting for the game thread, in % of the capacity.
static ENERGY: Mutex<Vec<i32>> = Mutex::new(Vec::new());
pub fn queue_energy(pct: i32) { if pct > 0 { ENERGY.lock().unwrap_or_else(|e| e.into_inner()).push(pct); } }
/// Game thread, every frame: the queued EN refills on the player's AC (current EN, up to the capacity).
pub unsafe fn apply_energy() {
    let q: Vec<i32> = core::mem::take(&mut *ENERGY.lock().unwrap_or_else(|e| e.into_inner()));
    if q.is_empty() { return; }
    let chr = player_chr(mem::module_base());
    let mods = if chr == 0 { 0 } else { mem::rd_usize(chr + o::PLAYER_MODULES) };
    let en = if mods == 0 { 0 } else { mem::rd_usize(mods + 8 * o::CHR_MOD_EN) };
    if en == 0 { return; }
    let cap = f32::from_bits(mem::rd_u32(en + o::EN_CAPACITY));
    let mut cur = f32::from_bits(mem::rd_u32(en + o::EN_CURRENT));
    for pct in q { cur = (cur + cap * pct as f32 / 100.0).min(cap); }
    mem::wr_u32(en + o::EN_CURRENT, cur.to_bits());
    logf!("chips: player EN refilled to {:.0} / {:.0}", cur, cap);
}

/// Game thread, every frame (penalty.rs frame hook): apply the queued SpEffects to the player's AC through the game's
/// own call (exe+968EA0: the character's SpEffect manager [chr+0x308], id, target, source, parameters, three flags),
/// the player being both the target and the source. Kept queued while there is no player character.
pub unsafe fn apply_speffects(base: usize) {
    let mut q = SPEFFECTS.lock().unwrap_or_else(|e| e.into_inner());
    if q.is_empty() { return; }
    let chr = player_chr(base);
    let mgr = if chr == 0 { 0 } else { mem::rd_usize(chr + o::CHR_SPEFFECT_MGR) };
    if mgr == 0 || mem::rd_usize(mgr) != base + o::SPEFFECT_MGR_VTABLE { return; }
    type FnApply = unsafe extern "system" fn(mgr: usize, id: i32, target: usize, source: usize, params: *const [f32; 4], a6: u32, a7: u32, a8: u32) -> u64;
    let apply: FnApply = core::mem::transmute(base + o::FN_APPLY_SPEFFECT);
    let params = [1.0f32; 4];
    q.retain(|&(id, not)| {
        if chr == not { return true; }
        let r = apply(mgr, id, chr, chr, &params, 0, 0, 0);
        logf!("speffect {} applied to the player {:#X} (ret {:#X})", id, chr, r);
        false
    });
}

/// The game reports a character's AP change (exe+1714F00 from its update, `FN_AP_CHANGED`): AP 0 now and above 0 last
/// frame, on another character than the player = an enemy destroyed (`enemy_destroyed`, `e.max_ap` = its max AP).
unsafe extern "system" fn ap_changed_hook(chr: usize) -> u64 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize) -> u64 = core::mem::transmute(base + o::FN_AP_CHANGED);
    let mods = if chr == 0 { 0 } else { mem::rd_usize(chr + o::PLAYER_MODULES) };
    let blk = if mods == 0 { 0 } else { mem::rd_usize(mods) };
    let before = if blk == 0 { (0, 0, 0) } else { (mem::rd_i32(blk + o::CHR_AP), mem::rd_i32(blk + o::CHR_AP_PREV), mem::rd_i32(blk + o::CHR_AP_MAX)) };
    let r = orig(chr);
    if blk != 0 {
        let (ap, prev, max) = before;
        if ap <= 0 && prev > 0 && chr != player_chr(base) {
            logf!("enemy destroyed: character {:#X} (max AP {})", chr, max);
            let pos = |k: usize| f32::from_bits(mem::rd_u32(chr + o::CHR_POS + 4 * k)) as f64;
            emit_with(Event::EnemyDestroyed, &[("max_ap", max as f64), ("tier", config::enemy_tier(max) as f64), ("x", pos(0)), ("y", pos(1)), ("z", pos(2))]);
        }
    }
    r
}
/// Boot: the event hooks (only the ones a chip uses).
pub unsafe fn install(base: usize) {
    let used = |e: Event| chip::chips().iter().any(|c| c.script_on.contains(&e) || c.on.iter().any(|h| h.0 == e));
    if used(Event::EnemyDestroyed) {
        match install_call_hook(base, o::AP_CHANGED_CALLSITE, o::FN_AP_CHANGED, ap_changed_hook as *const () as usize) {
            Ok(_) => log("events: enemy_destroyed hook installed"),
            Err(e) => logf!("events: enemy_destroyed hook FAILED: {}", e),
        }
    }
    if used(Event::EnemyStaggered) {
        for &site in &o::STAGGER_SET_CALLSITES {
            match install_call_hook(base, site, o::FN_STAGGER_SET, stagger_set_hook as *const () as usize) {
                Ok(_) => logf!("events: enemy_staggered hook installed at exe+{:X}", site),
                Err(e) => logf!("events: enemy_staggered hook at exe+{:X} FAILED: {}", site, e),
            }
        }
    }
    if used(Event::CoreExpansion) {
        for &site in &o::SPEFFECT_ADD_CALLSITES {
            match install_call_hook(base, site, o::FN_SPEFFECT_ADD, speffect_add_hook as *const () as usize) {
                Ok(_) => logf!("events: core_expansion hook installed at exe+{:X}", site),
                Err(e) => logf!("events: core_expansion hook at exe+{:X} FAILED: {}", site, e),
            }
        }
    }
}

/// A character's stagger state for this frame (exe+870B40: its stagger module, staggered byte): the byte going from 0
/// to 1 on another character than the player = an enemy staggered (`enemy_staggered`, `e.max_ap` = its max AP).
unsafe extern "system" fn stagger_set_hook(module: usize, staggered: u8) -> u64 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, u8) -> u64 = core::mem::transmute(base + o::FN_STAGGER_SET);
    let was = mem::rd_u8(module + o::STAGGER_STATE);
    let r = orig(module, staggered);
    if was == 0 && staggered != 0 {
        let chr = mem::rd_usize(module + o::MODULE_OWNER);
        let mods = if chr == 0 { 0 } else { mem::rd_usize(chr + o::PLAYER_MODULES) };
        if mods != 0 && mem::rd_usize(mods + 8 * o::STAGGER_MODULE) == module && chr != player_chr(base) {
            let blk = mem::rd_usize(mods);
            let max = if blk == 0 { 0 } else { mem::rd_i32(blk + o::CHR_AP_MAX) };
            logf!("enemy staggered: character {:#X} (max AP {})", chr, max);
            emit_with(Event::EnemyStaggered, &[("max_ap", max as f64), ("tier", config::enemy_tier(max) as f64)]);
        }
    }
    r
}

/// A SpEffect added to a character (exe+96B6C0: manager, id, ...; the 4 callers of the game's add): the player's
/// manager getting CORE_EXPANSION_SPEFFECT = a core expansion firing (`core_expansion`).
unsafe extern "system" fn speffect_add_hook(mgr: usize, id: i32, a3: usize, a4: usize, a5: u32, a6: f32, a7: u32, a8: u32) -> u64 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, i32, usize, usize, u32, f32, u32, u32) -> u64 = core::mem::transmute(base + o::FN_SPEFFECT_ADD);
    let r = orig(mgr, id, a3, a4, a5, a6, a7, a8);
    if id == o::CORE_EXPANSION_SPEFFECT {
        let chr = player_chr(base);
        if chr != 0 && mgr == mem::rd_usize(chr + o::CHR_SPEFFECT_MGR) {
            log("core expansion fired (player)");
            emit(Event::CoreExpansion);
        }
    }
    r
}

/// An OS Tuning / chip row by number or name.
pub fn row_of(t: &str) -> Option<u16> {
    if let Ok(r) = t.trim().parse::<u16>() { return os::tuning(r).map(|x| x.row); }
    let k = config::name_key(t);
    os::tunings().iter().find(|x| config::name_key(&os::name(x.row)) == k).map(|x| x.row)
}

/// Game thread, garage (penalty.rs frame hook): apply the queued COAM change and gifts; their dialog lines wait for
/// the garage menu (`open_dialog`).
pub unsafe fn run_garage(game: &Game, gd: usize) {
    let (credits, give, mut lines) = {
        let mut q = GARAGE.lock().unwrap_or_else(|e| e.into_inner());
        if q.credits == 0 && q.give.is_empty() && q.lines.is_empty() { return; }
        (core::mem::take(&mut q.credits), core::mem::take(&mut q.give), core::mem::take(&mut q.lines))
    };
    if credits != 0 {
        let now = mem::rd_i32(gd + o::GD_COAM) as i64;
        let v = (now + credits).clamp(0, 999_999_999);
        mem::wr_i32(gd + o::GD_COAM, v as i32);
        logf!("event actions: COAM {} -> {}", now, v);
    }
    for name in give {
        match penalty::item_by_name(&name) {
            Some(id) => { let given = penalty::give_copy(game, gd, id); logf!("event actions: give {} ({:#X}): {}", name, id, given);
                if given { lines.push(format!("Pièce obtenue : {}", penalty::part_name(id))); } }
            None => logf!("event actions: give {}: no such part", name),
        }
    }
    if !lines.is_empty() {
        let mut d = DIALOG.lock().unwrap_or_else(|e| e.into_inner());
        if d.0.is_empty() { d.1 = msg::GARAGE_TITLE_SEEN.load(Relaxed); }
        logf!("event actions: dialog waits for the garage menu (title lookups so far {})", d.1);
        d.0.extend(lines);
    }
}

/// Game thread, garage: the queued dialog, once the garage title was looked up since the lines were queued.
pub fn open_dialog() {
    let mut d = DIALOG.lock().unwrap_or_else(|e| e.into_inner());
    if d.0.is_empty() || msg::GARAGE_TITLE_SEEN.load(Relaxed) <= d.1 { return; }
    logf!("event actions: dialog shown (garage title lookups {})", msg::GARAGE_TITLE_SEEN.load(Relaxed));
    dialog::show(&core::mem::take(&mut d.0).join("\n"));
}
