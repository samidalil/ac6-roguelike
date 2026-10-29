//! Our own OS chips: one file per chip in `chips/` next to the DLL (the defaults in dll/src/chips/ are written when
//! the folder is missing). A chip is offered and leveled like an OS Tuning (os.rs: reward grid, [part_rarity],
//! [lost_on_failure], [max_occurrences] by its row 9000 + id or its name); its level is saved in event flags
//! (CHIP_LEVELS). Effects are declarative and typed (`[effect]` stat = "+" / "-": the level's value in %), read from
//! the levels when used (like the game's OS Tunings) and applied to the player's AC only through hooks on its stat code
//! (the part params are not touched, so Assemblage shows the parts' own stats): max AP, EN capacity and total weight
//! (recomputed by the game every frame), and each weapon's ammunition when the game fills it (mission load). RE_NOTES
//! "OS Tuning values and own OS chips". Script hooks (SpEffect...) come later; resupply is not covered yet.

use super::*;
use std::sync::OnceLock;

/// Effect types a chip file can use (`[effect]` keys).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stat { Ap, EnCapacity, Weight, Ammo }
const STATS: [(&str, Stat); 4] = [("ap", Stat::Ap), ("en_capacity", Stat::EnCapacity), ("weight", Stat::Weight), ("ammo", Stat::Ammo)];

pub struct Chip {
    /// 1..MAX_CHIPS: its save slot and row (CHIP_ROW0 + id)
    pub id: u16,
    pub name: String,
    /// stat named on the reward tile ("PB +5 %")
    pub stat: String,
    /// effect value per level (1..CHIP_MAX_LEVEL levels)
    pub values: Vec<i32>,
    effects: Vec<(Stat, i32)>,
}
impl Chip {
    pub fn row(&self) -> u16 { o::CHIP_ROW0 + self.id }
    /// sign of the stat change shown on the tile (the first effect's)
    pub fn sign(&self) -> i32 { self.effects.first().map_or(1, |e| e.1) }
}

static CHIPS: OnceLock<Vec<Chip>> = OnceLock::new();
pub fn chips() -> &'static [Chip] { CHIPS.get().map_or(&[], |v| v.as_slice()) }
pub fn chip(row: u16) -> Option<&'static Chip> { chips().iter().find(|c| c.row() == row) }

const DEFAULTS: [(&str, &str); 4] = [
    ("reinforced_plating.toml", include_str!("chips/reinforced_plating.toml")),
    ("overclocked_generator.toml", include_str!("chips/overclocked_generator.toml")),
    ("light_alloys.toml", include_str!("chips/light_alloys.toml")),
    ("extended_magazines.toml", include_str!("chips/extended_magazines.toml")),
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
        match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
            Ok(c) if v.iter().any(|x| x.id == c.id) => logf!("chips: {}: id {} already used, left out", name, c.id),
            Ok(c) => v.push(c),
            Err(e) => logf!("chips: {}: {}, left out", name, e),
        }
    }
    v.sort_by_key(|c| c.id);
    logf!("chips: {}", v.iter().map(|c| format!("{} {} ({} {:?}, {:?})", c.row(), c.name, c.stat, c.values, c.effects))
        .collect::<Vec<_>>().join(", "));
    let _ = CHIPS.set(v);
}

fn parse(text: &str) -> Result<Chip, String> {
    use config::Value;
    let mut c = Chip { id: 0, name: String::new(), stat: String::new(), values: Vec::new(), effects: Vec::new() };
    let mut section = String::new();
    for (n, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = config::strip_comment(raw).trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') { section = line[1..line.len() - 1].trim().to_string(); continue; }
        let Some((k, v)) = line.rsplit_once('=') else { return Err(format!("line {}: no `key = value`", n + 1)) };
        let (k, v) = (k.trim().trim_matches('"').trim(), v.trim());
        let val = config::parse_value(v).ok_or_else(|| format!("line {}: {} is not a value", n + 1, v))?;
        match (section.as_str(), k, &val) {
            ("", "id", Value::Num(id)) if (1..=o::MAX_CHIPS as u32).contains(id) => c.id = *id as u16,
            ("", "name", Value::Text(t)) => c.name = t.clone(),
            ("", "stat", Value::Text(t)) => c.stat = t.clone(),
            ("", "levels", _) => c.values = config::parse_levels(&val, o::CHIP_MAX_LEVEL)
                .ok_or_else(|| format!("line {}: levels = \"value/value/...\" (1 to {} levels) expected", n + 1, o::CHIP_MAX_LEVEL))?,
            ("effect", _, _) => {
                let sign = match &val { Value::Text(t) if t.trim() == "+" => 1, Value::Text(t) if t.trim() == "-" => -1, Value::Num(1) => 1,
                    _ => return Err(format!("line {}: {} = \"+\" or \"-\" expected", n + 1, k)) };
                let &(_, stat) = STATS.iter().find(|s| s.0.eq_ignore_ascii_case(k))
                    .ok_or_else(|| format!("line {}: unknown effect {} ({})", n + 1, k, STATS.map(|s| s.0).join(", ")))?;
                c.effects.push((stat, sign));
            }
            _ => return Err(format!("line {}: unknown setting [{}] {} = {}", n + 1, section, k, v)),
        }
    }
    if c.id == 0 { return Err(format!("id = 1..{} missing", o::MAX_CHIPS)); }
    if c.name.is_empty() || c.values.is_empty() { return Err("name and levels are needed".into()); }
    Ok(c)
}

// ---- levels (event flags, saved with the run; New Game clears block 4)
fn flags() -> Option<(Game, usize)> {
    let g = unsafe { Game::new(mem::module_base()) };
    let man = unsafe { g.flag_man()? };
    Some((g, man))
}
fn level_flag(c: &Chip) -> u32 { o::CHIP_LEVELS + (c.id as u32 - 1) * o::CHIP_LEVEL_BITS }
pub unsafe fn level(row: u16) -> Option<i32> {
    let c = chip(row)?;
    let (g, man) = flags()?;
    Some(g.get_bits(man, level_flag(c), o::CHIP_LEVEL_BITS) as i32)
}
/// New level (0..max) of a chip (pick at the commit, loss on a failure).
pub unsafe fn set_level(row: u16, lv: i32) -> bool {
    let (Some(c), Some((g, man))) = (chip(row), flags()) else { return false };
    g.put_bits(man, level_flag(c), o::CHIP_LEVEL_BITS, lv.clamp(0, c.values.len() as i32) as u32);
    true
}
/// [max_occurrences] used up (CHIP_GONE + id - 1).
pub unsafe fn gone(g: &Game, man: usize, row: u16) -> bool { chip(row).is_some_and(|c| g.get_flag(man, o::CHIP_GONE + c.id as u32 - 1) == Some(true)) }
pub unsafe fn set_gone(g: &Game, man: usize, row: u16) { if let Some(c) = chip(row) { g.put_flag(man, o::CHIP_GONE + c.id as u32 - 1, true); } }

/// Effect value of a chip at a level (0: none).
pub fn value(row: u16, lv: i32) -> i32 { chip(row).and_then(|c| c.values.get((lv - 1).max(0) as usize).copied()).filter(|_| lv > 0).unwrap_or(0) }

// ---- effects
/// Change in % of a stat from the chip levels, read from the save's flags when the hook needs it (several chips on one
/// stat add up), the way the game reads its own OS Tuning levels when it uses them: nothing to refresh on a pick, a
/// failure, Continue or New Game. 0 without a save.
unsafe fn pct(base: usize, s: Stat) -> i32 {
    let man = mem::rd_usize(base + o::EVENTFLAGMAN_GLOBAL);
    if man == 0 { return 0; }
    let g = Game::new(base);
    let mut p = 0;
    for c in chips().iter().filter(|c| c.effects.iter().any(|e| e.0 == s)) {
        let lv = g.get_bits(man, level_flag(c), o::CHIP_LEVEL_BITS) as usize;
        if lv == 0 { continue; }
        let v = c.values[lv.min(c.values.len()) - 1];
        for &(st, sign) in &c.effects { if st == s { p += sign * v; } }
    }
    p
}
fn scaled(v: f64, p: i32) -> f64 { v * (100 + p).max(0) as f64 / 100.0 }

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
    let v = if block != 0 && block == player_module(base, 0) { scaled(value as f64, pct(base, Stat::Ap)).round() as i32 } else { value };
    let orig: unsafe extern "system" fn(usize, i32) -> u64 = core::mem::transmute(base + o::FN_SET_MAX_AP);
    orig(block, v)
}
/// EN capacity of an EN object (every frame): the player's gets the EN chips' %.
unsafe extern "system" fn en_capacity_hook(en: usize) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize) -> f32 = core::mem::transmute(base + o::FN_EN_CAPACITY);
    let v = orig(en);
    if en != 0 && en == player_module(base, o::CHR_MOD_EN) { scaled(v as f64, pct(base, Stat::EnCapacity)) as f32 } else { v }
}
/// Total weight of an assembly (every frame, for a character's movement): the player's gets the weight chips' %.
unsafe extern "system" fn total_weight_hook(assembly: usize) -> f32 {
    let base = mem::module_base();
    let orig: unsafe extern "system" fn(usize) -> f32 = core::mem::transmute(base + o::FN_TOTAL_WEIGHT);
    let v = orig(assembly);
    if assembly == 0 { return v; }
    let gdm = mem::rd_usize(base + o::GAMEDATAMAN_GLOBAL);
    let gd = if gdm == 0 { 0 } else { mem::rd_usize(gdm + o::GDM_PLAYER_GAMEDATA) };
    if gd != 0 && assembly == gd + o::GD_ASSEMBLY { scaled(v as f64, pct(base, Stat::Weight)) as f32 } else { v }
}

/// Boot: the stat hooks (call sites of the game's per-frame stat code). Not installed without chips.
pub unsafe fn install(base: usize) {
    if chips().is_empty() { return; }
    let mut hooks: Vec<(usize, usize, usize)> = o::SET_MAX_AP_CALLSITES.iter().map(|&s| (s, o::FN_SET_MAX_AP, set_max_ap_hook as *const () as usize)).collect();
    hooks.push((o::EN_CAPACITY_CALLSITE, o::FN_EN_CAPACITY, en_capacity_hook as *const () as usize));
    hooks.push((o::TOTAL_WEIGHT_CALLSITE, o::FN_TOTAL_WEIGHT, total_weight_hook as *const () as usize));
    hooks.push((o::ADD_AMMO_CALLSITES[0], o::FN_ADD_AMMO, add_ammo_hook as *const () as usize));
    hooks.push((o::ADD_AMMO_CALLSITES[1], o::FN_ADD_AMMO, add_ammo_hook as *const () as usize));
    for (site, target, f) in hooks {
        match install_call_hook(base, site, target, f) {
            Ok(_) => {}
            Err(e) => logf!("chips: hook at exe+{:X} FAILED: {}", site, e),
        }
    }
    log("chips: stat hooks installed (max AP, EN capacity, total weight, weapon ammunition)");
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
    let p = pct(base, Stat::Ammo);
    if p == 0 { return r; }
    let Some(&total) = AMMO.get().and_then(|t| t.get(&id)) else { return r };
    if reserve < 0 || loaded < 0 || loaded + reserve != total { return r; }
    let new = (scaled(total as f64, p).round() as i32 - loaded).max(0);
    mem::wr_i32(slot + o::SLOT_RESERVE, new);
    // the game refills the weapons many times while a mission opens (each refill is raised again): one log per slot
    static LOGGED: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    if !logged.contains(&slot) {
        if logged.len() >= 64 { logged.clear(); }
        logged.push(slot);
        logf!("chips: weapon {} ammunition {} -> {} ({:+}%)", id, total, loaded + new, p);
    }
    r
}
unsafe extern "system" fn add_ammo_hook(ammo: usize, rounds: i32) -> u64 { add_ammo(ammo, rounds) }
