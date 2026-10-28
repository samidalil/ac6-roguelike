//! Player-editable settings: `ac6_roguelike.toml` next to the DLL, read once at boot. Written with the defaults
//! (and comments) when missing. A tiny TOML subset: `[section]`, `key = integer`, quoted keys, `#` comments.

use super::*;
use std::sync::OnceLock;

pub struct Config {
    /// % of the game's mission pay kept (0 = no pay: selling is the only income)
    pub mission_pay_percent: u32,
    /// % of the arena's repeat-fight pay kept
    pub arena_repeat_pay_percent: u32,
    /// frame parts / internals sell for this % of their value
    pub part_sell_percent: u32,
    /// every part costs this % of its value: shop buy price, and the price of a starter bought after a failure
    pub item_charge_percent: u32,
    /// weapons sell for this % of their value
    pub weapon_sell_percent: u32,
    /// [prices]: part value overrides, (part name or equipId as written, value)
    pub prices: Vec<(String, u32)>,
    /// % of the game's ammo and repair costs charged after a mission
    pub ammo_cost_percent: u32,
    pub repair_cost_percent: u32,
    /// shop size: weapons and other parts offered after each cleared mission (total at most MAX_SHOP_SLOTS)
    pub shop_weapons: u32,
    pub shop_parts: u32,
    /// parts given at the end of each cleared mission
    pub parts_per_mission: u32,
    /// parts offered per reward (pick one, 1..12; 1 = the part is given directly)
    pub reward_choices: u32,
    /// [rewards] core_expansions: the four core expansions can be mission rewards (unique, unlocked when picked)
    pub core_expansions: bool,
    /// cleared missions between shop rerolls (1 = every mission)
    pub reroll_interval: u32,
    /// pay of the first mission of a run (MissionParam 2010, Illegal Entry): the run's starting money
    pub starting_credits: u32,
    /// rarity tier (1 common, 2 rare, 3 epic) of every part [part_rarity] does not list
    pub unlisted_tier: u32,
    /// odds of each tier when the shop or a reward picks a part
    pub common_weight: u32,
    pub rare_weight: u32,
    pub epic_weight: u32,
    /// [part_rarity]: tier overrides, (part name or equipId as written, tier 1..3)
    pub part_rarity: Vec<(String, u32)>,
}

pub const MAX_SHOP_SLOTS: u32 = 8;

fn defaults() -> Config {
    Config {
        mission_pay_percent: 0, arena_repeat_pay_percent: 0, part_sell_percent: 25, weapon_sell_percent: 25,
        item_charge_percent: 100, prices: Vec::new(), ammo_cost_percent: 100, repair_cost_percent: 100,
        shop_weapons: 1, shop_parts: 2, parts_per_mission: 1, reward_choices: 3, core_expansions: true, reroll_interval: 1, starting_credits: 100_000, unlisted_tier: 1,
        common_weight: 60, rare_weight: 30, epic_weight: 10, part_rarity: Vec::new(),
    }
}

/// The settings file written when missing (dll/src/default_config.toml, embedded at build time).
const DEFAULT_FILE: &str = include_str!("default_config.toml");

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config { CONFIG.get_or_init(defaults) }

/// Name key used to match [prices] entries with part names: upper case, no spaces.
pub fn name_key(s: &str) -> String { s.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_uppercase).collect() }

/// Path of the settings file: the DLL's own folder.
fn path() -> std::path::PathBuf {
    let mut buf = [0u16; 1024];
    let mut module: usize = 0;
    unsafe {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | UNCHANGED_REFCOUNT
        GetModuleHandleExW(0x4 | 0x2, path as *const () as *const u16, &mut module);
        let n = GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) as usize;
        let p = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
        p.parent().map(|d| d.join("ac6_roguelike.toml")).unwrap_or_else(|| "ac6_roguelike.toml".into())
    }
}

extern "system" {
    fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut usize) -> i32;
    fn GetModuleFileNameW(module: usize, buf: *mut u16, size: u32) -> u32;
}

pub fn load() {
    let p = path();
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(_) => {
            let _ = std::fs::write(&p, DEFAULT_FILE);
            logf!("config: {} not found, written with the defaults", p.display());
            DEFAULT_FILE.to_string()
        }
    };
    let text = text.trim_start_matches('\u{feff}'); // a byte-order mark left by some editors
    let mut c = defaults();
    let mut section = String::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') { section = line[1..line.len() - 1].trim().to_string(); continue; }
        let Some((k, v)) = line.rsplit_once('=') else { logf!("config line {}: ignored ({})", n + 1, raw); continue };
        let (k, v) = (k.trim().trim_matches('"').trim(), v.trim());
        let Ok(v) = v.replace('_', "").parse::<u32>() else { logf!("config line {}: {} is not a number", n + 1, v); continue };
        match (section.as_str(), k) {
            ("economy", "mission_pay_percent") => c.mission_pay_percent = v,
            ("economy", "arena_repeat_pay_percent") => c.arena_repeat_pay_percent = v,
            ("shop", "part_sell_percent") => c.part_sell_percent = v,
            ("shop", "weapon_sell_percent") => c.weapon_sell_percent = v,
            ("shop", "item_charge_percent") => c.item_charge_percent = v,
            ("prices", _) => c.prices.push((k.to_string(), v)),
            ("economy", "ammo_cost_percent") => c.ammo_cost_percent = v,
            ("economy", "repair_cost_percent") => c.repair_cost_percent = v,
            ("shop", "shop_weapons") => c.shop_weapons = v,
            ("shop", "shop_parts") => c.shop_parts = v,
            ("rewards", "parts_per_mission") => c.parts_per_mission = v.min(3),
            ("rewards", "reward_choices") => c.reward_choices = v.clamp(1, 12),
            ("rewards", "core_expansions") => c.core_expansions = v != 0,
            ("shop", "reroll_interval") => c.reroll_interval = v.clamp(1, 255), // 8-bit counter in the save
            ("economy", "starting_credits") => c.starting_credits = v.min(999_999_999),
            ("rarity", "unlisted_tier") => c.unlisted_tier = v.clamp(1, 3),
            ("rarity", "common_weight") => c.common_weight = v,
            ("rarity", "rare_weight") => c.rare_weight = v,
            ("rarity", "epic_weight") => c.epic_weight = v,
            ("part_rarity", _) => c.part_rarity.push((k.to_string(), v.clamp(1, 3))),
            _ => logf!("config line {}: unknown setting [{}] {}", n + 1, section, k),
        }
    }
    if c.shop_weapons + c.shop_parts > MAX_SHOP_SLOTS {
        logf!("config: shop_weapons + shop_parts > {}, keeping the first {}", MAX_SHOP_SLOTS, MAX_SHOP_SLOTS);
        c.shop_weapons = c.shop_weapons.min(MAX_SHOP_SLOTS);
        c.shop_parts = MAX_SHOP_SLOTS - c.shop_weapons;
    }
    logf!("config: ammo cost {}%, repair cost {}%, starting credits {}, shop {} weapon(s) + {} part(s) rerolled every {} mission(s), {} reward(s) per mission, unlisted tier {} weights {}/{}/{}, {} rarity override(s)",
        c.ammo_cost_percent, c.repair_cost_percent, c.starting_credits, c.shop_weapons, c.shop_parts, c.reroll_interval, c.parts_per_mission,
        c.unlisted_tier, c.common_weight, c.rare_weight, c.epic_weight, c.part_rarity.len());
    logf!("config {}: mission pay {}%, arena repeat pay {}%, parts sell {}%, weapons sell {}%, charge {}%, {} price override(s)",
        p.display(), c.mission_pay_percent, c.arena_repeat_pay_percent, c.part_sell_percent,
        c.weapon_sell_percent, c.item_charge_percent, c.prices.len());
    let _ = CONFIG.set(c);
}
