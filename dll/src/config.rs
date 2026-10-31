//! Player-editable settings: `ac6_roguelike.toml` next to the DLL, read once at boot. Written with the defaults
//! (and comments) when missing. A tiny TOML subset: `[section]`, `key = value` (integer, 0x hex, quoted or bare
//! string), quoted keys, `#` comments (outside quotes).

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
    /// [rewards] os_rewards: OS Tunings are mission rewards (a level per pick) and the OS Tuning menu stays locked
    pub os_rewards: bool,
    /// cleared missions between shop rerolls (1 = every mission)
    pub reroll_interval: u32,
    /// pay of the first mission of a run (MissionParam 2010, Illegal Entry): the run's starting money
    pub starting_credits: u32,
    /// [pool.NAME] sections, in file order, plus the internal unlisted pool (always there)
    pub pools: Vec<Pool>,
    /// [rarity] unlisted_pool: index in `pools` of the pool that gets every part [part_rarity] does not list
    pub unlisted_pool: usize,
    /// [part_rarity]: (part name, equipId or OsReinforce row as written, index in `pools`; None = `= 0`, out of the
    /// pools)
    pub part_rarity: Vec<(String, Option<usize>)>,
    /// [lost_on_failure]: (part name, equipId or OsReinforce row as written, 1 = the equipped item is lost on a failed
    /// sortie, 0 = it survives); unlisted: weapons and frame parts / internals lost, core expansions kept
    pub lost_on_failure: Vec<(String, u32)>,
    /// [max_occurrences]: same keys (any part or OS Tuning), N = offered at most N times per run (reward grid or shop,
    /// picked or not; 1..MAX_OCCURRENCES), 0 = no limit like unlisted
    pub max_occurrences: Vec<(String, u32)>,
    /// [os_tuning]: (OsReinforce row, effect value per level, 1..5 levels: the tuning's max level); unlisted rows keep
    /// the game's values
    pub os_tuning: Vec<(u16, Vec<i32>)>,
    /// [inventory] max_parts: parts owned at most (weapons, frame parts, internals; equipped ones and each copy count)
    pub max_parts: u32,
    /// [enemy_tiers]: (name, lowest max AP), sorted by that AP; an enemy is in the last tier its max AP reaches
    pub enemy_tiers: Vec<(String, u32)>,
}

/// A pool of parts: the shop and the rewards pick a pool by weight (among the pools with a candidate left), then a
/// part uniformly in it. Weight 0 = its parts are never offered.
#[derive(Clone, Debug)]
pub struct Pool {
    /// section name as written ([pool.NAME]), matched through `name_key`
    pub name: String,
    /// text shown on the reward tiles (upper case; default the name)
    pub label: String,
    pub weight: u32,
    /// 0xRRGGBB of the label
    pub color: u32,
    /// the reward tiles show the label
    pub show_name: bool,
}
/// Name of the internal pool of the parts [part_rarity] does not list.
pub const UNLISTED: &str = "unlisted";
impl Pool {
    fn new(name: &str) -> Pool { Pool { name: name.to_string(), label: name.to_uppercase(), weight: 0, color: 0xFFFFFF, show_name: true } }
    /// the unlisted pool, or a pool [rarity] unlisted_pool names without a section: out, grey, no label
    fn unlisted(name: &str) -> Pool { Pool { color: 0x808080, show_name: false, ..Pool::new(name) } }
}

/// Pools at most, the unlisted one included (u8 index in lib.rs `Part`, 0xFF = out).
pub const MAX_POOLS: usize = 200;

pub const MAX_SHOP_SLOTS: u32 = o::SHOP_SLOTS_MAX;
/// [inventory] max_parts at least the frame parts / internals an AC needs (7; weapon slots may stay empty).
pub const MIN_PARTS: u32 = 7;
/// [max_occurrences] values above this are read as this (the save counts the offers below the max in OCCUR_COUNT_BITS)
pub const MAX_OCCURRENCES: u32 = 1 << o::OCCUR_COUNT_BITS;

fn defaults() -> Config {
    Config {
        mission_pay_percent: 0, arena_repeat_pay_percent: 0, part_sell_percent: 25, weapon_sell_percent: 25,
        item_charge_percent: 100, prices: Vec::new(), ammo_cost_percent: 100, repair_cost_percent: 100,
        shop_weapons: 1, shop_parts: 2, parts_per_mission: 1, reward_choices: 3, os_rewards: true, reroll_interval: 1, starting_credits: 100_000,
        pools: vec![Pool::unlisted(UNLISTED)], unlisted_pool: 0, part_rarity: Vec::new(),
        lost_on_failure: Vec::new(), max_occurrences: Vec::new(), os_tuning: Vec::new(), max_parts: 20, enemy_tiers: Vec::new(),
    }
}

/// The settings file written when missing (dll/src/default_config.toml, embedded at build time).
const DEFAULT_FILE: &str = include_str!("default_config.toml");

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config { CONFIG.get_or_init(defaults) }

/// Name key used to match [prices] entries with part names: upper case, no spaces.
pub fn name_key(s: &str) -> String { s.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_uppercase).collect() }

/// Value of a per-item table ([lost_on_failure], [max_occurrences]) for an item known by its numeric keys
/// (equipId, OsReinforce row) and names; None = not listed. Keys are matched like [part_rarity] (numbers as written,
/// names through `name_key`); the first matching entry wins.
pub fn value(table: &[(String, u32)], ids: &[u32], names: &[String]) -> Option<u32> {
    let names: Vec<String> = names.iter().map(|n| name_key(n)).collect();
    table.iter().find(|(k, _)| match k.parse::<u32>() { Ok(id) => ids.contains(&id), Err(_) => names.contains(&name_key(k)) }).map(|e| e.1)
}
/// `value` of a 1/0 table.
pub fn lookup(table: &[(String, u32)], ids: &[u32], names: &[String]) -> Option<bool> { value(table, ids, names).map(|v| v != 0) }

/// Index in `pools` of the pool called `name` (`name_key` match).
pub fn pool_index(pools: &[Pool], name: &str) -> Option<usize> { pools.iter().position(|p| name_key(&p.name) == name_key(name)) }

/// A setting's value: a number (decimal with `_` separators, or 0x hex) or a string (quoted, or a bare word).
pub(crate) enum Value { Num(u32), Text(String) }
pub(crate) fn parse_value(v: &str) -> Option<Value> {
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') { return Some(Value::Text(v[1..v.len() - 1].to_string())); }
    let n = v.replace('_', "");
    if let Some(h) = n.strip_prefix("0x").or_else(|| n.strip_prefix("0X")) { return u32::from_str_radix(h, 16).ok().map(Value::Num); }
    if n.starts_with(|c: char| c.is_ascii_digit()) { return n.parse::<u32>().ok().map(Value::Num); }
    (!v.is_empty() && !v.contains('"')).then(|| Value::Text(v.to_string()))
}
/// A colour: 0xRRGGBB or "#RRGGBB".
fn parse_color(v: &Value) -> Option<u32> {
    match v {
        Value::Num(n) => Some(*n & 0xFF_FFFF),
        Value::Text(t) => t.trim().strip_prefix('#').filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()),
    }
}
/// A line without its `#` comment (a `#` inside quotes is text: "#RRGGBB").
pub(crate) fn strip_comment(raw: &str) -> &str {
    let mut quoted = false;
    for (i, c) in raw.char_indices() {
        match c { '"' => quoted = !quoted, '#' if !quoted => return &raw[..i], _ => {} }
    }
    raw
}

/// Values per level written "a/b/c" (or one number): 1..`max` integers, None when malformed.
pub(crate) fn parse_levels(v: &Value, max: usize) -> Option<Vec<i32>> {
    let v: Vec<i32> = match v {
        Value::Num(n) => vec![*n as i32],
        Value::Text(t) => t.split('/').map(|x| x.trim().parse::<i32>().ok()).collect::<Option<Vec<_>>>()?,
    };
    (!v.is_empty() && v.len() <= max).then_some(v)
}

/// The DLL's own folder (settings file, chips/).
pub fn dir() -> std::path::PathBuf { path().parent().map(|d| d.to_path_buf()).unwrap_or_else(|| ".".into()) }

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
    let mut unlisted_pool = UNLISTED.to_string();
    // [part_rarity] values as written: pool names are resolved once every [pool.NAME] is read
    let mut rarity: Vec<(String, Value, usize)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            // [pool.NAME]: a pool (the unlisted one is already there)
            if let Some(name) = section.strip_prefix("pool.").map(|s| s.trim().trim_matches('"').trim().to_string()) {
                if name.is_empty() { logf!("config line {}: [pool.] without a name", n + 1); }
                else if pool_index(&c.pools, &name).is_some() {
                    if name_key(&name) != name_key(UNLISTED) { logf!("config line {}: [pool.{}] defined twice, settings merged", n + 1, name); }
                } else if c.pools.len() >= MAX_POOLS { logf!("config line {}: more than {} pools, [pool.{}] ignored", n + 1, MAX_POOLS, name); }
                else { c.pools.push(Pool::new(&name)); }
                section = format!("pool.{}", name);
            }
            continue;
        }
        let Some((k, v)) = line.rsplit_once('=') else { logf!("config line {}: ignored ({})", n + 1, raw); continue };
        let (k, v) = (k.trim().trim_matches('"').trim(), v.trim());
        let Some(val) = parse_value(v) else { logf!("config line {}: {} is not a value", n + 1, v); continue };
        if let Some(pool) = section.strip_prefix("pool.").and_then(|name| pool_index(&c.pools, name)) {
            let p = &mut c.pools[pool];
            match (k, &val) {
                ("weight", Value::Num(w)) => p.weight = *w,
                ("show_name", Value::Num(b)) => p.show_name = *b != 0,
                ("label", Value::Text(t)) => p.label = t.to_uppercase(),
                ("color", _) => match parse_color(&val) {
                    Some(rgb) => p.color = rgb,
                    None => logf!("config line {}: {} is not a colour (\"#RRGGBB\" or 0xRRGGBB)", n + 1, v),
                },
                _ => logf!("config line {}: unknown pool setting [{}] {} = {}", n + 1, section, k, v),
            }
            continue;
        }
        match (section.as_str(), k, &val) {
            ("rarity", "unlisted_pool", Value::Text(t)) => { unlisted_pool = t.clone(); continue }
            ("part_rarity", _, _) => { rarity.push((k.to_string(), val, n + 1)); continue }
            ("os_tuning", _, _) => {
                match (k.parse::<u16>(), parse_levels(&val, o::OSR_LEVELS)) {
                    (Ok(row), Some(v)) => c.os_tuning.push((row, v)),
                    _ => logf!("config line {}: [os_tuning] {} = {}: an OsReinforce row = \"value/value/...\" (1 to {} levels) expected", n + 1, k, v, o::OSR_LEVELS),
                }
                continue
            }
            _ => {}
        }
        let Value::Num(v) = val else { logf!("config line {}: {} is not a number", n + 1, v); continue };
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
            ("rewards", "os_rewards") => c.os_rewards = v != 0,
            ("shop", "reroll_interval") => c.reroll_interval = v.clamp(1, 255), // 8-bit counter in the save
            ("economy", "starting_credits") => c.starting_credits = v.min(999_999_999),
            ("inventory", "max_parts") => c.max_parts = v.max(MIN_PARTS), // a complete frame must fit
            ("lost_on_failure", _) => c.lost_on_failure.push((k.to_string(), v.min(1))),
            ("enemy_tiers", _) => c.enemy_tiers.push((k.to_string(), v)),
            ("max_occurrences", _) => {
                if v > MAX_OCCURRENCES { logf!("config line {}: [max_occurrences] {} = {} read as {}", n + 1, k, v, MAX_OCCURRENCES); }
                c.max_occurrences.push((k.to_string(), v.min(MAX_OCCURRENCES)))
            }
            _ => logf!("config line {}: unknown setting [{}] {}", n + 1, section, k),
        }
    }
    if c.enemy_tiers.is_empty() {
        c.enemy_tiers = [("small", 0), ("medium", 1000), ("large", 10000), ("boss", 30000)].iter().map(|(n, v)| (n.to_string(), *v)).collect();
    }
    c.enemy_tiers.sort_by_key(|t| t.1);
    // the pool of the unlisted parts: created like the unlisted one when no section defines it
    c.unlisted_pool = pool_index(&c.pools, &unlisted_pool).unwrap_or_else(|| {
        logf!("config: [rarity] unlisted_pool = {}: no [pool.{}], created (weight 0: its parts are not offered)", unlisted_pool, unlisted_pool);
        c.pools.push(Pool::unlisted(&unlisted_pool));
        c.pools.len() - 1
    });
    for (k, val, line) in rarity {
        let pool = match &val {
            Value::Num(0) => None,
            Value::Text(t) => match pool_index(&c.pools, t) {
                Some(i) => Some(i),
                None => { logf!("config line {}: [part_rarity] {} = {}: no such pool, the part goes to the unlisted pool", line, k, t); Some(c.unlisted_pool) }
            },
            Value::Num(x) => { logf!("config line {}: [part_rarity] {} = {}: a pool name or 0 expected, the part goes to the unlisted pool", line, k, x); Some(c.unlisted_pool) }
        };
        c.part_rarity.push((k, pool));
    }
    if c.shop_weapons + c.shop_parts > MAX_SHOP_SLOTS {
        logf!("config: shop_weapons + shop_parts > {}, keeping the first {}", MAX_SHOP_SLOTS, MAX_SHOP_SLOTS);
        c.shop_weapons = c.shop_weapons.min(MAX_SHOP_SLOTS);
        c.shop_parts = MAX_SHOP_SLOTS - c.shop_weapons;
    }
    logf!("config: ammo cost {}%, repair cost {}%, starting credits {}, shop {} weapon(s) + {} part(s) rerolled every {} mission(s), {} reward(s) per mission, {} part_rarity entr(ies), {} lost-on-failure / {} max-occurrence / {} os_tuning entr(ies)",
        c.ammo_cost_percent, c.repair_cost_percent, c.starting_credits, c.shop_weapons, c.shop_parts, c.reroll_interval, c.parts_per_mission,
        c.part_rarity.len(), c.lost_on_failure.len(), c.max_occurrences.len(), c.os_tuning.len());
    logf!("config pools (unlisted parts -> {}): {}", c.pools[c.unlisted_pool].name, c.pools.iter().map(|p| format!("{} weight {} #{:06X}{}",
        p.name, p.weight, p.color, if p.show_name { format!(" \"{}\"", p.label) } else { String::new() })).collect::<Vec<_>>().join(", "));
    logf!("config {}: mission pay {}%, arena repeat pay {}%, parts sell {}%, weapons sell {}%, charge {}%, {} price override(s)",
        p.display(), c.mission_pay_percent, c.arena_repeat_pay_percent, c.part_sell_percent,
        c.weapon_sell_percent, c.item_charge_percent, c.prices.len());
    let _ = CONFIG.set(c);
}

/// Tier of an enemy by its max AP: 1 = the first [enemy_tiers] tier (lowest AP), ...
pub fn enemy_tier(max_ap: i32) -> usize {
    get().enemy_tiers.iter().rposition(|t| max_ap.max(0) as u32 >= t.1).map_or(1, |i| i + 1)
}
