//! Our own OS chips: one file per chip in `chips/` next to the DLL (the defaults in dll/src/chips/ are written when
//! the folder is missing). A chip is offered and leveled like an OS Tuning (os.rs: reward grid, [part_rarity],
//! [lost_on_failure], [max_occurrences] by its row 9000 + id or its name); its level is saved in event flags
//! (CHIP_LEVELS). Effects are declarative: `[effect]` modifier target = "+" / "-" (the level's value in %) or "add" /
//! "sub" (the value itself), read from the levels when the target is used (like the game's OS Tunings). Targets are
//! the engine's lookup points (`Target`, `modify`): the player's AC through hooks on its stat code (max AP, EN capacity
//! and total weight, recomputed by the game every frame; each weapon's ammunition when the game fills it; the part
//! params are not touched, so Assemblage shows the parts' own stats), and the mod's own rolls (pool weights, rewards
//! per mission, parts per reward, shop size). RE_NOTES "OS Tuning values and own OS chips" and "Modifier targets".
//! A chip may also have a Lua script next to its file (chips/<name>.lua, script.rs); resupply is not covered yet.

use super::*;
use std::sync::OnceLock;

/// Modifier targets: the values the engine looks up through `modify` where it uses them (`[effect]` keys). Pool
/// weights are `pool.<NAME>.weight` ([pool.NAME] sections).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target { Ap, EnCapacity, Weight, Ammo, QuickBoostEn, PoolWeight(u8), RewardCount, RewardChoices, ShopWeapons, ShopParts }
const TARGETS: [(&str, Target); 9] = [("ap", Target::Ap), ("en_capacity", Target::EnCapacity), ("weight", Target::Weight),
    ("ammo", Target::Ammo), ("quick_boost_en", Target::QuickBoostEn), ("reward.count", Target::RewardCount), ("reward.choices", Target::RewardChoices),
    ("shop.weapons", Target::ShopWeapons), ("shop.parts", Target::ShopParts)];
fn target(key: &str) -> Option<Target> {
    if let Some(name) = key.strip_prefix("pool.").and_then(|r| r.strip_suffix(".weight")) {
        return config::pool_index(&config::get().pools, name).map(|i| Target::PoolWeight(i as u8));
    }
    TARGETS.iter().find(|t| t.0.eq_ignore_ascii_case(key)).map(|t| t.1)
}

/// How an effect applies the level's value: in % of the target, or added as is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode { Percent, Flat }

pub struct Chip {
    /// 1..MAX_CHIPS: its save slot and row (CHIP_ROW0 + id)
    pub id: u16,
    pub name: String,
    /// stat named on the reward tile ("PB +5 %")
    pub stat: String,
    /// effect value per level (1..CHIP_MAX_LEVEL levels)
    pub values: Vec<i32>,
    effects: Vec<(Target, Mode, i32)>,
    /// `[on.<event>]` actions (event.rs)
    pub on: Vec<(event::Event, event::Action)>,
    /// `unit` shown after the level's value on the tile (default: " %", or nothing for add / sub effects or a chip
    /// with actions only)
    unit: Option<String>,
    /// chips/<name>.lua next to the chip file: (file name, source), and the events it has an `on_<event>` for
    pub script: Option<(String, String)>,
    pub script_on: Vec<event::Event>,
    /// `unsafe = true`: the script gets the `unsafe` table (raw memory)
    pub unsafe_api: bool,
}
impl Chip {
    pub fn row(&self) -> u16 { o::CHIP_ROW0 + self.id }
    /// sign of the stat change shown on the tile (the first effect's)
    pub fn sign(&self) -> i32 { self.effects.first().map_or(1, |e| e.2) }
    /// unit shown on the tile after the level's value (the first effect's mode)
    pub fn unit(&self) -> String {
        if let Some(u) = &self.unit { return u.clone(); }
        if self.effects.is_empty() || self.effects[0].1 == Mode::Flat { String::new() } else { " %".into() }
    }
}

static CHIPS: OnceLock<Vec<Chip>> = OnceLock::new();
pub fn chips() -> &'static [Chip] { CHIPS.get().map_or(&[], |v| v.as_slice()) }
pub fn chip(row: u16) -> Option<&'static Chip> { chips().iter().find(|c| c.row() == row) }

const DEFAULTS: [(&str, &str); 16] = [
    ("reinforced_plating.toml", include_str!("chips/reinforced_plating.toml")),
    ("overclocked_generator.toml", include_str!("chips/overclocked_generator.toml")),
    ("light_alloys.toml", include_str!("chips/light_alloys.toml")),
    ("extended_magazines.toml", include_str!("chips/extended_magazines.toml")),
    ("preferred_contract.toml", include_str!("chips/preferred_contract.toml")),
    ("performance_bonus.toml", include_str!("chips/performance_bonus.toml")),
    ("nanorepair.toml", include_str!("chips/nanorepair.toml")),
    ("streak_bonus.toml", include_str!("chips/streak_bonus.toml")),
    ("streak_bonus.lua", include_str!("chips/streak_bonus.lua")),
    ("salvage.toml", include_str!("chips/salvage.toml")),
    ("salvage.lua", include_str!("chips/salvage.lua")),
    ("last_stand.toml", include_str!("chips/last_stand.toml")),
    ("last_stand.lua", include_str!("chips/last_stand.lua")),
    ("amplified_impact.toml", include_str!("chips/amplified_impact.toml")),
    ("efficient_thrusters.toml", include_str!("chips/efficient_thrusters.toml")),
    ("adrenaline.toml", include_str!("chips/adrenaline.toml")),
];

/// Boot (after the settings): read every chips/*.toml. A file with an error is left out (log).
pub fn load() {
    let dir = config::dir().join("chips");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
        for (f, text) in DEFAULTS { let _ = std::fs::write(dir.join(f), text); }
        logf!("chips: {} not found, written with the {} default chips", dir.display(), DEFAULTS.len());
    }
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml"))).collect()).unwrap_or_default();
    files.sort();
    let mut v: Vec<Chip> = Vec::new();
    for f in files {
        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| parse(&t)).map(|c| with_script(c, &f)) {
            Ok(c) if v.iter().any(|x| x.id == c.id) => logf!("chips: {}: id {} already used, left out", name, c.id),
            Ok(c) => v.push(c),
            Err(e) => logf!("chips: {}: {}, left out", name, e),
        }
    }
    v.sort_by_key(|c| c.id);
    logf!("chips: {}", v.iter().map(|c| format!("{} {} ({} {:?}, {:?}{}{})", c.row(), c.name, c.stat, c.values, c.effects,
        if c.on.is_empty() { String::new() } else { format!(", on {:?}", c.on) },
        if c.script.is_none() { String::new() } else { format!(", script on {:?}", c.script_on) }))
        .collect::<Vec<_>>().join(", "));
    let _ = CHIPS.set(v);
}

/// The chip's script (chips/<name>.lua), if any, and the events it handles (`on_<event>` names in the source).
fn with_script(mut c: Chip, toml: &std::path::Path) -> Chip {
    let lua = toml.with_extension("lua");
    let Ok(src) = std::fs::read_to_string(&lua) else { return c };
    let ident = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    c.script_on = event::EVENTS.iter().filter(|(n, _)| {
        let f = format!("on_{}", n);
        src.match_indices(&f).any(|(i, _)| !src[..i].ends_with(ident) && !src[i + f.len()..].starts_with(ident))
    }).map(|e| e.1).collect();
    c.script = Some((lua.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), src));
    c
}

fn parse(text: &str) -> Result<Chip, String> {
    use config::Value;
    let mut c = Chip { id: 0, name: String::new(), stat: String::new(), values: Vec::new(), effects: Vec::new(), on: Vec::new(),
        unit: None, script: None, script_on: Vec::new(), unsafe_api: false };
    let mut section = String::new();
    for (n, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = config::strip_comment(raw).trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') { section = line[1..line.len() - 1].trim().to_string(); continue; }
        // [speffect.<row id>]: a SpEffectParam row for tools/regulation (mod/regulation.bin), not read here
        if section.starts_with("speffect.") { continue; }
        let Some((k, v)) = line.rsplit_once('=') else { return Err(format!("line {}: no `key = value`", n + 1)) };
        let (k, v) = (k.trim().trim_matches('"').trim(), v.trim());
        let val = config::parse_value(v).ok_or_else(|| format!("line {}: {} is not a value", n + 1, v))?;
        match (section.as_str(), k, &val) {
            ("", "id", Value::Num(id)) if (1..=o::MAX_CHIPS as u32).contains(id) => c.id = *id as u16,
            ("", "name", Value::Text(t)) => c.name = t.clone(),
            ("", "stat", Value::Text(t)) => c.stat = t.clone(),
            ("", "unit", Value::Text(t)) => c.unit = Some(t.clone()),
            ("", "unsafe", Value::Text(t)) if t == "true" || t == "false" => c.unsafe_api = t == "true",
            ("", "unsafe", Value::Num(n)) => c.unsafe_api = *n != 0,
            (s, _, _) if s.starts_with("on.") => {
                let e = event::event(&s[3..]).ok_or_else(|| format!("line {}: unknown event [{}] ({})", n + 1, s,
                    event::EVENTS.map(|e| e.0).join(", ")))?;
                c.on.push((e, event::Action::parse(k, &val).map_err(|m| format!("line {}: {}", n + 1, m))?));
            }
            ("", "levels", _) => c.values = config::parse_levels(&val, o::CHIP_MAX_LEVEL)
                .ok_or_else(|| format!("line {}: levels = \"value/value/...\" (1 to {} levels) expected", n + 1, o::CHIP_MAX_LEVEL))?,
            ("effect", _, _) => {
                let (mode, sign) = match &val {
                    Value::Text(t) if t.trim() == "+" => (Mode::Percent, 1), Value::Text(t) if t.trim() == "-" => (Mode::Percent, -1),
                    Value::Text(t) if t.trim().eq_ignore_ascii_case("add") => (Mode::Flat, 1),
                    Value::Text(t) if t.trim().eq_ignore_ascii_case("sub") => (Mode::Flat, -1),
                    _ => return Err(format!("line {}: {} = \"+\", \"-\", \"add\" or \"sub\" expected", n + 1, k)) };
                let t = target(k).ok_or_else(|| format!("line {}: unknown target {} ({}, pool.<NAME>.weight)", n + 1, k,
                    TARGETS.map(|t| t.0).join(", ")))?;
                c.effects.push((t, mode, sign));
            }
            _ => return Err(format!("line {}: unknown setting [{}] {} = {}", n + 1, section, k, v)),
        }
    }
    if c.id == 0 { return Err(format!("id = 1..{} missing", o::MAX_CHIPS)); }
    if c.name.is_empty() || c.values.is_empty() { return Err("name and levels are needed".into()); }
    Ok(c)
}

// ---- levels (event flags CHIP_LEVELS, saved with the run; New Game clears them)
fn flags() -> Option<(Game, usize)> {
    let g = unsafe { Game::new(mem::module_base()) };
    let man = unsafe { g.flag_man()? };
    Some((g, man))
}
fn level_flag(c: &Chip) -> u32 { o::CHIP_LEVELS + (c.id as u32 - 1) * o::CHIP_LEVEL_BITS }
pub unsafe fn level(row: u16) -> Option<i32> {
    let (g, man) = flags()?;
    level_in(&g, man, row)
}
/// `level` with the flag manager already found (many reads in a row).
pub unsafe fn level_in(g: &Game, man: usize, row: u16) -> Option<i32> {
    let c = chip(row)?;
    Some(g.get_bits(man, level_flag(c), o::CHIP_LEVEL_BITS) as i32)
}
/// New level (0..max) of a chip (pick at the commit, loss on a failure). At 0 its script's saved byte is cleared too
/// (a chip picked again starts afresh).
pub unsafe fn set_level(row: u16, lv: i32) -> bool {
    let (Some(c), Some((g, man))) = (chip(row), flags()) else { return false };
    let lv = lv.clamp(0, c.values.len() as i32);
    g.put_bits(man, level_flag(c), o::CHIP_LEVEL_BITS, lv as u32);
    if lv == 0 { g.put_bits(man, o::SCRIPT_STORE + (c.id as u32 - 1) * o::SCRIPT_STORE_BITS, o::SCRIPT_STORE_BITS, 0); }
    true
}

/// Effect value of a chip at a level (0: none).
pub fn value(row: u16, lv: i32) -> i32 { chip(row).and_then(|c| c.values.get((lv - 1).max(0) as usize).copied()).filter(|_| lv > 0).unwrap_or(0) }

// ---- modifiers
/// Changes of a target from the chip levels (in %, and flat), read from the save's flags when the target is used
/// (several chips on one target add up), the way the game reads its own OS Tuning levels when it uses them: nothing to
/// refresh on a pick, a failure, Continue or New Game. (0, 0) without a save.
unsafe fn changes(base: usize, t: Target) -> (i32, i32) {
    if !chips().iter().any(|c| c.effects.iter().any(|e| e.0 == t)) { return (0, 0); }
    let man = mem::rd_usize(base + o::EVENTFLAGMAN_GLOBAL);
    if man == 0 { return (0, 0); }
    let g = Game::new(base);
    let (mut pct, mut flat) = (0, 0);
    for c in chips().iter().filter(|c| c.effects.iter().any(|e| e.0 == t)) {
        let lv = g.get_bits(man, level_flag(c), o::CHIP_LEVEL_BITS) as usize;
        if lv == 0 { continue; }
        let v = c.values[lv.min(c.values.len()) - 1];
        for &(et, mode, sign) in &c.effects {
            if et != t { continue; }
            match mode { Mode::Percent => pct += sign * v, Mode::Flat => flat += sign * v }
        }
    }
    (pct, flat)
}
/// A target's value after the chips: `v` x (100 + the % changes) / 100 + the flat changes, never below 0.
pub unsafe fn modify(t: Target, v: f64) -> f64 {
    let (pct, flat) = changes(mem::module_base(), t);
    if pct == 0 && flat == 0 { return v; }
    (v * (100 + pct).max(0) as f64 / 100.0 + flat as f64).max(0.0)
}
/// `modify` for a count, rounded and kept within `lo..=hi`.
pub unsafe fn modify_count(t: Target, v: u32, lo: u32, hi: u32) -> u32 { (modify(t, v as f64).round() as u32).clamp(lo, hi) }

/// The player's character modules (WorldChrMan player -> +0x320), 0 when there is no player character.
unsafe fn player_modules(base: usize) -> usize {
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    if wcm == 0 { return 0; }
    let chr = mem::rd_usize(wcm + o::WCM_PLAYER);
    if chr == 0 { return 0; }
    mem::rd_usize(chr + o::PLAYER_MODULES)
}
unsafe fn player_module(base: usize, k: usize) -> usize {
    let m = player_modules(base);
    if m == 0 { 0 } else { mem::rd_usize(m + 8 * k) }
}

/// Max AP setter (every frame, every character): the player's AP block gets the AP chips' %.
unsafe extern "system" fn set_max_ap_hook(block: usize, value: i32) -> u64 {
    let base = mem::module_base();
    let v = if block != 0 && block == player_module(base, 0) { modify(Target::Ap, value as f64).round() as i32 } else { value };
    let orig: unsafe extern "system" fn(usize, i32) -> u64 = core::mem::transmute(base + o::FN_SET_MAX_AP);
    orig(block, v)
}
/// EN capacity of an EN object (every frame): the player's gets the EN chips' %.
unsafe extern "system" fn en_capacity_hook(en: usize) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize) -> f32 = core::mem::transmute(base + o::FN_EN_CAPACITY);
    let v = orig(en);
    if en != 0 && en == player_module(base, o::CHR_MOD_EN) { modify(Target::EnCapacity, v as f64) as f32 } else { v }
}
/// Total weight of an assembly (every frame, for a character's movement): the player's gets the weight chips' %.
unsafe extern "system" fn total_weight_hook(assembly: usize) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize) -> f32 = core::mem::transmute(base + o::FN_TOTAL_WEIGHT);
    let v = orig(assembly);
    if assembly == 0 { return v; }
    let gdm = mem::rd_usize(base + o::GAMEDATAMAN_GLOBAL);
    let gd = if gdm == 0 { 0 } else { mem::rd_usize(gdm + o::GDM_PLAYER_GAMEDATA) };
    if gd != 0 && assembly == gd + o::GD_ASSEMBLY { modify(Target::Weight, v as f64).max(1.0) as f32 } else { v }
}

/// Boot: the stat hooks (call sites of the game's per-frame stat code). Not installed without chips.
pub unsafe fn install(base: usize) {
    if chips().is_empty() { return; }
    let mut hooks: Vec<(usize, usize, usize)> = o::SET_MAX_AP_CALLSITES.iter().map(|&s| (s, o::FN_SET_MAX_AP, set_max_ap_hook as *const () as usize)).collect();
    hooks.push((o::EN_CAPACITY_CALLSITE, o::FN_EN_CAPACITY, en_capacity_hook as *const () as usize));
    hooks.push((o::TOTAL_WEIGHT_CALLSITE, o::FN_TOTAL_WEIGHT, total_weight_hook as *const () as usize));
    hooks.push((o::ADD_AMMO_CALLSITES[0], o::FN_ADD_AMMO, add_ammo_hook as *const () as usize));
    hooks.push((o::ADD_AMMO_CALLSITES[1], o::FN_ADD_AMMO, add_ammo_hook as *const () as usize));
    for &s in &o::RESUPPLY_CALLSITES { hooks.push((s, o::FN_RESUPPLY, resupply_hook as *const () as usize)); }
    hooks.push((o::QUICK_BOOST_EN_JMPSITE, o::FN_EN_SPEND, quick_boost_en_hook as *const () as usize));
    hooks.push((o::EN_RECOVERY_GROSS_CALLSITE, o::FN_EN_RECOVERY_GROSS, en_recovery_gross_hook as *const () as usize));
    hooks.push((o::EN_RECOVERY_NET_CALLSITE, o::FN_EN_RECOVERY_NET, en_recovery_net_hook as *const () as usize));
    for (site, target, f) in hooks {
        match install_call_hook(base, site, target, f) {
            Ok(_) => {}
            Err(e) => logf!("chips: hook at exe+{:X} FAILED: {}", site, e),
        }
    }
    log("chips: stat hooks installed (max AP, EN capacity, total weight, weapon ammunition, quick boost EN, EN recovery)");
}

/// Total ammunition of each weapon (EquipParamWeapon totalBulletNum by equipId), read once at boot.
static AMMO: OnceLock<std::collections::HashMap<u32, i32>> = OnceLock::new();
pub unsafe fn init_ammo(base: usize) {
    if AMMO.get().is_some() { return; }
    let Some(f) = param_index(base, "EquipParamWeapon").and_then(|i| param_file(base, i)) else { return };
    let m = param_rows(f).into_iter().map(|(id, r)| (id, mem::rd_i32(r + o::WEAPON_TOTAL_AMMO))).filter(|x| x.1 > 0).collect();
    let _ = AMMO.set(m);
}

/// The game adds ammunition to a weapon (exe+907750: rcx = the slot's ammunition block, edx = rounds; the loaded
/// round first, then the reserve, capped at the weapon's total): when a player weapon ends up full (mission load),
/// its reserve gets the ammo chips' %. The slot's module is found back through the slot stride (checked by its
/// vtable), its owner at module + 8.
unsafe fn add_ammo(ammo: usize, rounds: i32) -> u64 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, i32) -> u64 = core::mem::transmute(base + o::FN_ADD_AMMO);
    let r = orig(ammo, rounds);
    if ammo < o::SLOT_AMMO_BLOCK + o::WEAPON_SLOT0 { return r; }
    let slot = ammo - o::SLOT_AMMO_BLOCK;
    let Some(module) = (0..o::WEAPON_SLOTS).map(|k| slot.wrapping_sub(o::WEAPON_SLOT0 + k * o::WEAPON_SLOT_STRIDE))
        .find(|&m| mem::rd_usize(m) == base + o::WEAPONS_VTABLE) else { return r };
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    let player = if wcm == 0 { 0 } else { mem::rd_usize(wcm + o::WCM_PLAYER) };
    let (id, loaded, reserve) = (mem::rd_u32(slot + o::SLOT_ID), mem::rd_i32(slot + o::SLOT_LOADED), mem::rd_i32(slot + o::SLOT_RESERVE));
    if player == 0 || mem::rd_usize(module + o::WEAPONS_OWNER) != player { return r; }
    let full = modify(Target::Ammo, 1e6);
    if full == 1e6 { return r; }
    let Some(&total) = AMMO.get().and_then(|t| t.get(&id)) else { return r };
    if reserve < 0 || loaded < 0 || loaded + reserve != total { return r; }
    let new = (modify(Target::Ammo, total as f64).round() as i32 - loaded).max(0);
    mem::wr_i32(slot + o::SLOT_RESERVE, new);
    // the game refills the weapons many times while a mission opens (each refill is raised again): one log per slot
    static LOGGED: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    if !logged.contains(&slot) {
        if logged.len() >= 64 { logged.clear(); }
        logged.push(slot);
        logf!("chips: weapon {} ammunition {} -> {}", id, total, loaded + new);
    }
    r
}
unsafe extern "system" fn add_ammo_hook(ammo: usize, rounds: i32) -> u64 { add_ammo(ammo, rounds) }

/// Resupply (exe+907540: rcx = a character's weapon module, xmm1 = rate; each slot gets max(1, rate x total)
/// rounds, capped at the total). For the player's module, each weapon with a known total ends with what the game
/// would give with the raised total: the rounds the call put into the loaded round stay, the rest goes to the
/// reserve, capped at the raised total.
unsafe extern "system" fn resupply_hook(module: usize, rate: f32) -> u64 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, f32) -> u64 = core::mem::transmute(base + o::FN_RESUPPLY);
    if module == 0 || module != player_module(base, o::CHR_MOD_WEAPONS) { return orig(module, rate); }
    let slots: Vec<(usize, u32, i32, i32)> = (0..o::WEAPON_SLOTS).map(|k| module + o::WEAPON_SLOT0 + k * o::WEAPON_SLOT_STRIDE)
        .map(|s| (s, mem::rd_u32(s + o::SLOT_ID), mem::rd_i32(s + o::SLOT_LOADED), mem::rd_i32(s + o::SLOT_RESERVE))).collect();
    let r = orig(module, rate);
    let mut out = Vec::new();
    for (s, id, loaded, reserve) in slots {
        let Some(&total) = AMMO.get().and_then(|t| t.get(&id)) else { continue };
        if loaded < 0 || reserve < 0 { continue; }
        let raised = modify(Target::Ammo, total as f64).round() as i32;
        let now = mem::rd_i32(s + o::SLOT_LOADED);
        if raised != total && now >= 0 {
            let rest = ((raised as f32 * rate) as i32).max(1) - (now - loaded).max(0);
            mem::wr_i32(s + o::SLOT_RESERVE, (reserve + rest.max(0)).min(raised - now).max(0));
        }
        out.push(format!("{} {} -> {}", id, loaded + reserve, now + mem::rd_i32(s + o::SLOT_RESERVE)));
    }
    logf!("chips: resupply x{} of the player's weapons: {}", rate, out.join(", "));
    r
}

/// Quick boost EN (the jmp at the end of exe+88B130 to the EN spending exe+88C0E0): the player's quick boost cost gets
/// the quick_boost_en chips' %.
unsafe extern "system" fn quick_boost_en_hook(en: usize, amount: f32, a3: usize, one_off: u32) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, f32, usize, u32) -> f32 = core::mem::transmute(base + o::FN_EN_SPEND);
    let amount = if en != 0 && en == player_module(base, o::CHR_MOD_EN) { modify(Target::QuickBoostEn, amount as f64) as f32 } else { amount };
    orig(en, amount, a3, one_off)
}

/// The mod's own SpEffect rows (MOD_SPEFFECT_ROWS) on the player: their energyRecoverSpeedRate is moved from the
/// gross EN recovery (where the game applies it, before the load term: a 1.2 there gave about +80 % on a starter
/// AC) to the net one, so a row's rate is the recovery's real change. Vanilla rows are left alone.
const MOD_SPEFFECT_ROWS: core::ops::Range<i32> = 900_000_000..900_001_000;
static EN_MOD_RATE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F80_0000); // 1.0
unsafe fn player_mod_recover_rate(base: usize) -> f32 {
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    let chr = if wcm == 0 { 0 } else { mem::rd_usize(wcm + o::WCM_PLAYER) };
    let mgr = if chr == 0 { 0 } else { mem::rd_usize(chr + o::CHR_SPEFFECT_MGR) };
    if mgr == 0 || mem::rd_usize(mgr) != base + o::SPEFFECT_MGR_VTABLE { return 1.0; }
    let mut rate = 1.0f32;
    let mut e = mem::rd_usize(mgr + 8);
    for _ in 0..256 {
        if e == 0 { break; }
        // the game's own filter on active entries (exe+9664F0)
        if mem::rd_u32(e + 0x64) & 0x800C_0003 == 0 && MOD_SPEFFECT_ROWS.contains(&mem::rd_i32(e)) {
            let row = mem::rd_usize(e + 8);
            if row != 0 { rate *= f32::from_bits(mem::rd_u32(row + o::SP_ENERGY_RECOVER_RATE)); }
        }
        e = mem::rd_usize(e + 0x38);
    }
    rate
}
unsafe extern "system" fn en_recovery_gross_hook(en: usize, x: f32) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, f32) -> f32 = core::mem::transmute(base + o::FN_EN_RECOVERY_GROSS);
    let v = orig(en, x);
    if en == 0 || en != player_module(base, o::CHR_MOD_EN) { return v; }
    let r = player_mod_recover_rate(base);
    EN_MOD_RATE.store(r.to_bits(), Relaxed);
    if r > 0.0 && r != 1.0 { v / r } else { v }
}
unsafe extern "system" fn en_recovery_net_hook(en: usize, net: f32) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize, f32) -> f32 = core::mem::transmute(base + o::FN_EN_RECOVERY_NET);
    let r = f32::from_bits(EN_MOD_RATE.load(Relaxed));
    let net = if net > 0.0 && r > 0.0 && en != 0 && en == player_module(base, o::CHR_MOD_EN) { net * r } else { net };
    orig(en, net)
}
