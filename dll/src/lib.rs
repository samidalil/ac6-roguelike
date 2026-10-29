//! Armored Core VI roguelike — me3 native DLL.
//!
//! Mechanisms (all verified live, see RE_NOTES.md):
//! * SHOP: the shop builder calls a release predicate per lineup row every frame (exe+E89074). We
//!   detour that call to `shop_pred`, which returns 1 only for the 3 chosen rows. Copy-migration
//!   proof, flag-independent, scoped to the shop.
//! * MISSION SIGNAL: the item manager's DEFER byte (GameData+0x600+0x99) is 1 while a mission is
//!   active. 0->1 = mission start, 1->0 = the mission-end "obtained parts" commit.
//! * POOL: shop offers and rewards are drawn by rarity among every buyable part; weapons and frame parts can repeat
//!   and be owned several times (dupshop.rs lists an owned one in the shop). Boosters, generators and FCS are
//!   unique: never rolled while owned or once lost forever. `[max_occurrences]` caps the offers of an item per run
//!   (occur.rs). OS Tunings are rewards only: one level per pick, the
//!   vanilla OS Tuning menu locked (os.rs).
//! * REWARD: at mission start we grant one random part through the game's give wrapper
//!   with bypass-defer = 0, so it is queued and the vanilla "Pièces obtenues" dialog shows it at
//!   mission end and commits it on OK.
//! * REROLL: at the commit (shop closed) we pick 1 weapon + 2 parts and persist the 3 lineup row
//!   indices in event flags (saved in the .sl2), so quit + reload restores the same shop. Bought
//!   parts are hidden by the ownership filter (slot stays empty until the next reroll).

use std::ffi::c_void;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering::*};

mod mem;
mod offsets;
use offsets as o;

/// Master switch for anything that WRITES to game memory (hook, grants, flags).
const ENABLE_PATCHES: bool = true;
/// Extra logging of every predicate decision change (noisy).
const VERBOSE: bool = false;

// ---------------------------------------------------------------- logging
fn log_path() -> std::path::PathBuf {
    let dir = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&dir).join("ac6_roguelike_dll.log")
}

fn log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{} {}", stamp(), msg);
    }
}

fn stamp() -> String {
    // wall clock without pulling in chrono: seconds since boot is enough to correlate with pollers
    format!("[{:>9.3}]", tick_count() as f64 / 1000.0)
}

macro_rules! logf { ($($a:tt)*) => {{ let mut s = String::new(); let _ = write!(s, $($a)*); log(&s); }} }

// declared after logf! so the module can use it (macro_rules scope is textual)
mod penalty;
mod buyback;
mod msg;
mod dialog;
mod config;
mod trace;
mod rewardmenu;
mod choice;
mod dupshop;
mod os;
mod chip;
mod event;
mod script;
mod occur;
mod cap;

// ---------------------------------------------------------------- lineup param (read-only use)
struct Lineup {
    data: usize, // first row
}

impl Lineup {
    unsafe fn row(&self, i: usize) -> usize { self.data + i * o::ROW_STRIDE }
    unsafe fn equip_id(&self, i: usize) -> u32 { mem::rd_u32(self.row(i) + o::ROW_EQUIP_ID) }
    unsafe fn price(&self, i: usize) -> i32 { mem::rd_i32(self.row(i) + o::ROW_PRICE) }
    unsafe fn equip_type(&self, i: usize) -> u8 { mem::rd_u8(self.row(i) + o::ROW_EQUIP_TYPE) }
    unsafe fn reward_cond(&self, i: usize) -> u32 { mem::rd_u32(self.row(i) + o::ROW_REWARD_COND) }
}

/// Validate a candidate param data start by its two invariant row anchors (1.09).
unsafe fn anchors_ok(data: usize) -> bool {
    if !mem::is_readable(data, o::ROW_COUNT * o::ROW_STRIDE) { return false; }
    let lu = Lineup { data };
    lu.equip_id(74) == 50070400 && lu.equip_id(140) == 10000000
}

/// Vanilla rank / arena rewards (rewardCondition != 0: S-rank counts, arena) become ordinary parts: the game grants
/// only rows with a condition (the ~297 rows at 0 are never granted), so clearing it stops the grant, and the pool and
/// the price tables, which skip conditioned rows, take them in with their shop price. Script grants (the Ice Worm's
/// VE-60SNA) have no condition and are untouched.
unsafe fn release_rank_rewards(lu: &Lineup) {
    let mut n = 0;
    for i in 0..o::ROW_COUNT {
        let cond = lu.reward_cond(i);
        if cond == 0 { continue; }
        mem::wr_u32(lu.row(i) + o::ROW_REWARD_COND, 0);
        logf!("rank reward released: row {} equip {} (condition {}, price {})", i, lu.equip_id(i), cond, lu.price(i));
        n += 1;
    }
    if n > 0 { logf!("{} vanilla rank/arena reward(s) turned into shop parts", n); }
}

/// One buyable row, cached from the param (indices are stable for a given game version), or an OS Tuning reward
/// (`os` = its OsReinforce row, `idx` = OS_IDX + its index: no lineup row, `equip_id` = its core expansion item or 0).
#[derive(Clone, Copy)]
/// `pool` = index in config `pools` (POOL_OUT = `= 0` in [part_rarity]); `max` = [max_occurrences] offers per run,
/// 0 = no limit.
struct Part { idx: u16, equip_id: u32, kind: u8, pool: u8, os: u16, max: u8 }
const KIND_OS: u8 = 0xFF;
const OS_IDX: u16 = 1000;
/// Part::pool of a part [part_rarity] lists as 0: never offered.
const POOL_OUT: u8 = 0xFF;
impl Part {
    fn is_weapon(&self) -> bool { self.kind == 0 }
    /// OS Tuning: a reward only, one level per pick (os.rs)
    fn is_os(&self) -> bool { self.kind == KIND_OS }
    /// one copy at most: booster, generator, FCS (copies would share a handle)
    fn unique(&self) -> bool { matches!(self.kind, 4 | 5 | 6) }
    /// id accepted by the item-descriptor builder (grant path)
    fn grant_id(&self) -> u32 { if self.is_os() { self.equip_id } else { o::id_tag(self.kind) | (self.equip_id & 0x0FFF_FFFF) } }
    fn name(&self) -> String { if self.is_os() { os::name(self.os) } else { penalty::part_name(self.grant_id()) } }
    /// in the shop / reward pools: in a pool whose weight is not 0 (`= 0` in [part_rarity] = left out on purpose)
    fn in_pool(&self) -> bool { pool_weight(self.pool) > 0 }
    /// its pool's settings (None: out)
    fn pool(&self) -> Option<&'static config::Pool> { config::get().pools.get(self.pool as usize) }
    fn pool_name(&self) -> String { self.pool().map_or("out".to_string(), |p| p.name.clone()) }
    /// text and colour the reward tile shows for its pool (None: no label)
    fn pool_label(&self) -> Option<(String, u32)> { self.pool().filter(|p| p.show_name).map(|p| (p.label.clone(), p.color)) }
}

/// [pool.NAME] weight of a pool (0 for POOL_OUT), after the chips' `pool.<NAME>.weight` modifiers.
fn pool_weight(pool: u8) -> u32 {
    let w = config::get().pools.get(pool as usize).map_or(0, |p| p.weight);
    unsafe { chip::modify(chip::Target::PoolWeight(pool), w as f64).round() as u32 }
}

/// Every row a roguelike shop may offer: priced, not a mission reward, weapon / frame / internal, plus the OS Tunings
/// (rewards only) unless [rewards] os_rewards = 0. Each gets its pool from [part_rarity] (OS Tunings by OsReinforce
/// row id), else the [rarity] unlisted_pool, and its [max_occurrences] value (offers per run, 0 = no limit). `= 0`, or
/// a pool whose weight is 0, keeps the part out of the shop and reward pools (`Part::in_pool`); it stays in the list
/// for lookups (a shop slot rolled under another config, debug commands).
unsafe fn build_pool(lu: &Lineup) -> Vec<Part> {
    let cfg = config::get();
    let index = |p: &Option<usize>| p.map_or(POOL_OUT, |i| i as u8);
    let mut by_id: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();
    let mut by_name: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    for (k, p) in &cfg.part_rarity {
        match k.parse::<u32>() { Ok(id) => { by_id.insert(id, index(p)); } Err(_) => { by_name.insert(config::name_key(k), index(p)); } }
    }
    let unlisted = cfg.unlisted_pool as u8;
    let mut v = Vec::new();
    for i in 0..o::ROW_COUNT {
        let buyable = lu.price(i) > 0 && lu.reward_cond(i) == 0;
        let kind = lu.equip_type(i);
        if buyable && matches!(kind, 0 | 1 | 4 | 5 | 6) {
            let id = lu.equip_id(i);
            let item = o::id_tag(kind) | (id & 0x0FFF_FFFF);
            let mut pool = unlisted;
            if let Some(p) = by_id.get(&id) { pool = *p; }
            else if let Some(p) = by_name.get(&config::name_key(&penalty::part_name(item))) { pool = *p; }
            let max = if cfg.max_occurrences.is_empty() { 0 } else { config::value(&cfg.max_occurrences, &[id], &[penalty::part_name(item)]).unwrap_or(0) as u8 };
            v.push(Part { idx: i as u16, equip_id: id, kind, pool, os: 0, max });
        }
    }
    if cfg.os_rewards {
        for (k, t) in os::tunings().iter().enumerate() {
            let pool = by_id.get(&(t.row as u32)).copied().unwrap_or(unlisted);
            let max = config::value(&cfg.max_occurrences, &[t.row as u32, t.item], &[os::name(t.row), penalty::part_name(t.item)]).unwrap_or(0) as u8;
            v.push(Part { idx: OS_IDX + k as u16, equip_id: t.item, kind: KIND_OS, pool, os: t.row, max });
        }
    }
    let capped: Vec<String> = v.iter().filter(|p| p.max > 0).map(|p| format!("{} x{}", p.name(), p.max)).collect();
    if !capped.is_empty() { logf!("max occurrences (offers per run): {}", capped.join(", ")); }
    let pool = |f: &dyn Fn(&Part) -> bool| v.iter().filter(|p| p.in_pool() && f(p)).count();
    let sizes: Vec<String> = cfg.pools.iter().enumerate().map(|(i, p)| format!("{} {} (weight {})", p.name,
        v.iter().filter(|x| x.pool as usize == i).count(), p.weight)).collect();
    logf!("pools: {}, {} at 0", sizes.join(", "), v.iter().filter(|p| p.pool == POOL_OUT).count());
    let unlisted: Vec<String> = v.iter().filter(|p| p.pool == cfg.unlisted_pool as u8).map(|p| p.name()).collect();
    if !unlisted.is_empty() && unlisted.len() <= 40 { logf!("unlisted parts ({}): {}", cfg.pools[cfg.unlisted_pool].name, unlisted.join(", ")); }
    logf!("pools after exclusion: shop weapons {}, shop parts {}, rewards {} ({} OS Tunings); {} part(s) left out",
        pool(&|p| !p.is_os() && p.is_weapon()), pool(&|p| !p.is_os() && !p.is_weapon()), pool(&|_| true),
        pool(&|p| p.is_os()), v.iter().filter(|p| !p.in_pool()).count());
    v
}

/// Weighted pick: a pool by its [pool.NAME] weight (pools without candidates skipped), then uniform in it. Parts out
/// of the pools (`= 0`, weight 0) never come out; None when no candidate is left.
fn pick_weighted(rng: &mut Rng, cands: &[Part]) -> Option<Part> {
    let present: Vec<u8> = (0..config::get().pools.len() as u8).filter(|t| pool_weight(*t) > 0 && cands.iter().any(|p| p.pool == *t)).collect();
    let total: u64 = present.iter().map(|t| pool_weight(*t) as u64).sum();
    if total == 0 { return None; }
    let mut r = rng.next() % total;
    let mut pool = present[0];
    for t in &present { let w = pool_weight(*t) as u64; if r < w { pool = *t; break; } r -= w; }
    let in_pool: Vec<Part> = cands.iter().copied().filter(|p| p.pool == pool).collect();
    Some(in_pool[rng.below(in_pool.len())])
}

// ---------------------------------------------------------------- RNG
struct Rng(u64);
impl Rng {
    fn seeded() -> Self {
        let t = tick_count();
        let c = unsafe { core::arch::x86_64::_rdtsc() };
        Rng((t.rotate_left(17) ^ c ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize { if n == 0 { 0 } else { (self.next() % n as u64) as usize } }
}

// ---------------------------------------------------------------- game function bindings
type FnBuildDesc = unsafe extern "system" fn(ctx: usize, out: *mut u8, id: *const u32) -> *const u8;
// NOTE: bool/byte params are passed as full-width integers on purpose — with u8 the upper bits of
// the register/stack slot are unspecified on Win64 and the game read some "0" states as non-zero.
type FnGiveItem = unsafe extern "system" fn(obj: usize, desc: *const u8, qty: u32, a4: u32, a5: u32, a6: u32, a7: u32) -> i32;

struct Game {
    base: usize,
    build_desc: FnBuildDesc,
    give_item: FnGiveItem,
}

impl Game {
    unsafe fn new(base: usize) -> Game {
        Game {
            base,
            build_desc: core::mem::transmute(base + o::FN_BUILD_DESC),
            give_item: core::mem::transmute(base + o::FN_GIVE_ITEM),
        }
    }
    /// player GameData (None at boot / before the title screen finishes)
    unsafe fn game_data(&self) -> Option<usize> {
        let slot = self.base + o::GAMEDATAMAN_GLOBAL;
        if !mem::is_readable(slot, 8) { return None; }
        let gdm = mem::rd_usize(slot);
        if gdm == 0 || !mem::is_readable(gdm, 0x48) { return None; }
        let gd = mem::rd_usize(gdm + o::GDM_PLAYER_GAMEDATA);
        if gd == 0 || !mem::is_readable(gd + o::GD_ITEM_MGR, 0x100) { return None; }
        Some(gd)
    }
    /// `game_data` without VirtualQuery, for per-frame code on the game thread: GameDataMan lives as long as the game,
    /// and its player GameData pointer is 0 until a save is loaded.
    unsafe fn game_data_fast(&self) -> Option<usize> {
        let gdm = mem::rd_usize(self.base + o::GAMEDATAMAN_GLOBAL);
        if gdm == 0 { return None; }
        let gd = mem::rd_usize(gdm + o::GDM_PLAYER_GAMEDATA);
        if gd == 0 { None } else { Some(gd) }
    }
    unsafe fn flag_man(&self) -> Option<usize> {
        let slot = self.base + o::EVENTFLAGMAN_GLOBAL;
        if !mem::is_readable(slot, 8) { return None; }
        let m = mem::rd_usize(slot);
        if m == 0 || !mem::is_readable(m, 0x40) { return None; }
        Some(m)
    }
    /// Player (AP, max AP) during a mission; None when the chain is not built (garage, loading).
    unsafe fn player_ap(&self) -> Option<(i32, i32, usize)> {
        let slot = self.base + o::WORLDCHRMAN_GLOBAL;
        let wcm = mem::rd_usize(slot);
        if wcm == 0 || !mem::is_readable(wcm + o::WCM_PLAYER, 8) { return None; }
        let a = mem::rd_usize(wcm + o::WCM_PLAYER);
        if a == 0 || !mem::is_readable(a + o::PLAYER_MODULES, 8) { return None; }
        let b = mem::rd_usize(a + o::PLAYER_MODULES);
        if b == 0 || !mem::is_readable(b, 8) { return None; }
        let c = mem::rd_usize(b);
        if c == 0 || !mem::is_readable(c + o::CHR_AP, 8) { return None; }
        let (ap, max) = (mem::rd_i32(c + o::CHR_AP), mem::rd_i32(c + o::CHR_AP_MAX));
        if max <= 0 || ap > max { return None; }
        Some((ap, max, c))
    }
    /// Ask the game to write the save file (same bit the game's own RequestSave() sets). Atomic OR so we
    /// can never drop a bit the game thread sets at the same moment. Returns the mask before.
    unsafe fn request_save(&self) -> Option<u32> {
        let st = mem::rd_usize(self.base + o::SAVE_STATE_GLOBAL);
        if st == 0 || !mem::is_readable(st + o::SAVE_REQ_MASK, 4) { return None; }
        let a = &*((st + o::SAVE_REQ_MASK) as *const std::sync::atomic::AtomicU32);
        Some(a.fetch_or(1, std::sync::atomic::Ordering::SeqCst))
    }
    unsafe fn defer_byte(&self, gd: usize) -> u8 { mem::rd_u8(gd + o::GD_ITEM_MGR + o::MGR_DEFER) }
    /// The pending-obtained vector (4-byte gaitem handles) as a copy.
    unsafe fn pending(&self, gd: usize) -> Vec<u32> {
        let mgr = gd + o::GD_ITEM_MGR;
        let (b, e) = (mem::rd_usize(mgr + o::MGR_PENDING_BEGIN), mem::rd_usize(mgr + o::MGR_PENDING_END));
        if b == 0 || e < b || e - b > 0x1000 || !mem::is_readable(b, e - b) { return Vec::new(); }
        (0..(e - b) / 4).map(|i| mem::rd_u32(b + 4 * i)).collect()
    }

    /// Build the game's item descriptor for `id` into `out` (0x40 bytes is what the game reserves).
    unsafe fn desc(&self, id: u32, out: &mut [u8; 0x40]) -> *const u8 {
        let ctx = mem::rd_usize(self.base + o::ITEM_DESC_CTX);
        (self.build_desc)(ctx, out.as_mut_ptr(), &id)
    }
    /// Ownership by reading the gaitem table directly (16-byte entries {handle, id, count, ord},
    /// free slots = 0xFFFFFFFF). No game code is called, so this is safe from any thread at any time.
    unsafe fn owned_ids(&self, gd: usize) -> Vec<u32> {
        let mgr = gd + o::GD_ITEM_MGR;
        let table = mem::rd_usize(mgr + o::MGR_GAITEM_TABLE);
        let mut ids = Vec::new();
        if table == 0 { return ids; }
        // one readability query per 4 KB page, not per entry (VirtualQuery contends with the
        // game's own memory traffic during loads and made a scan take tens of seconds)
        let mut page_ok_until = 0usize;
        for i in 0..o::GAITEM_TABLE_MAX {
            let e = table + i * 16;
            if e + 16 > page_ok_until {
                if !mem::is_readable(e, 0x1000) { break; }
                page_ok_until = e + 0x1000;
            }
            let (handle, id) = (mem::rd_u32(e), mem::rd_u32(e + 4));
            if handle == 0xFFFF_FFFF { continue; }
            if handle == 0 && id == 0 { break; }
            ids.push(id);
        }
        ids
    }
    /// Grant through the vanilla give path with bypass-defer = 0. Returns (rax, error code).
    unsafe fn give_deferred(&self, gd: usize, id: u32) -> (i32, u32) {
        let mut buf = [0u8; 0x40];
        let d = self.desc(id, &mut buf);
        let obj = gd + o::GD_GIVE_OBJ;
        let r = (self.give_item)(obj, d, 1, 1, 1, 1, 0);
        (r, mem::rd_u32(obj + o::GIVEOBJ_ERROR))
    }

    // --- event flags by DIRECT bit-page access (no game calls: IsEventFlag/SetEventFlag take a
    // lock the game holds for ~20 s around mission clear, and contending there froze the game) ---
    /// Address of the bit page for `block` (= id / div) in CSEventFlagMan's block map (cached).
    unsafe fn flag_page(&self, man: usize, block: u32) -> Option<usize> {
        let slot = block as usize;
        if slot < PAGE_CACHE.len() && PAGE_CACHE_MAN.load(Relaxed) == man {
            let p = PAGE_CACHE[slot].load(Relaxed);
            if p != 0 { return Some(p); }
        }
        let p = self.flag_page_walk(man, block)?;
        if slot < PAGE_CACHE.len() {
            if PAGE_CACHE_MAN.load(Relaxed) != man { clear_page_cache(); PAGE_CACHE_MAN.store(man, Relaxed); }
            PAGE_CACHE[slot].store(p, Relaxed);
        }
        Some(p)
    }
    unsafe fn flag_page_walk(&self, man: usize, block: u32) -> Option<usize> {
        let bsz = mem::rd_u32(man + o::EFM_BLOCK_BYTES) as usize;
        let bbase = mem::rd_usize(man + o::EFM_PAGES_BASE);
        let hdr = mem::rd_usize(man + o::EFM_MAP_HEADER);
        if hdr == 0 || !mem::is_readable(hdr, 0x40) { return None; }
        let mut node = mem::rd_usize(hdr + 8);
        let mut best = hdr;
        let mut guard = 0;
        while node != 0 && mem::is_readable(node, 0x40) && mem::rd_u8(node + 0x19) != 1 && guard < 128 {
            guard += 1;
            if mem::rd_u32(node + 0x20) < block { node = mem::rd_usize(node + 0x10); }
            else { best = node; node = mem::rd_usize(node); }
        }
        if best == hdr || mem::rd_u32(best + 0x20) != block { return None; }
        match mem::rd_u32(best + 0x28) {
            1 => Some(bbase + bsz * mem::rd_u32(best + 0x30) as usize),
            2 => { let p = mem::rd_usize(best + 0x30); if p == 0 { None } else { Some(p) } }
            _ => None,
        }
    }
    /// (byte address, mask) of flag `id`; bits are MSB-first within a byte.
    unsafe fn flag_bit(&self, man: usize, id: u32) -> Option<(usize, u8)> {
        let div = mem::rd_u32(man + o::EFM_DIVISOR);
        if div == 0 { return None; }
        let page = self.flag_page(man, id / div)?;
        let rem = id % div;
        let addr = page + (rem / 8) as usize;
        Some((addr, 1u8 << (7 - (rem % 8))))
    }
    unsafe fn get_flag(&self, man: usize, id: u32) -> Option<bool> {
        let (a, m) = self.flag_bit(man, id)?;
        Some(mem::rd_u8(a) & m != 0)
    }
    unsafe fn put_flag(&self, man: usize, id: u32, v: bool) -> bool {
        match self.flag_bit(man, id) {
            Some((a, m)) => {
                let b = mem::rd_u8(a);
                mem::wr_u8(a, if v { b | m } else { b & !m });
                mem::wr_u8(man + o::EFM_DIRTY, 1); // what SetEventFlag does so the save picks it up
                true
            }
            None => false,
        }
    }

    /// Unsigned value stored in `bits` consecutive flags from `base`, LSB first.
    unsafe fn get_bits(&self, man: usize, base: u32, bits: u32) -> u32 {
        (0..bits).filter(|b| self.get_flag(man, base + b) == Some(true)).fold(0, |x, b| x | 1 << b)
    }
    unsafe fn put_bits(&self, man: usize, base: u32, bits: u32, v: u32) {
        for b in 0..bits { self.put_flag(man, base + b, (v >> b) & 1 != 0); }
    }

    // --- row bitsets: one flag per EquipmentLineupParam row index from `set` (o::ROWS_SOLD / LOST / ROLLED) ---
    /// Row indices present in the set.
    #[allow(dead_code)]
    unsafe fn read_rows(&self, man: usize, set: u32) -> Vec<u16> {
        (0..o::ROW_COUNT as u16).filter(|&i| self.get_flag(man, set + i as u32) == Some(true)).collect()
    }
    /// Replace the set with exactly `rows`.
    #[allow(dead_code)]
    unsafe fn write_rows(&self, man: usize, set: u32, rows: &[u16]) {
        for i in 0..o::ROW_COUNT as u16 { self.put_flag(man, set + i as u32, rows.contains(&i)); }
    }
    #[allow(dead_code)]
    unsafe fn clear_rows(&self, man: usize, set: u32) { self.write_rows(man, set, &[]); }
    /// Add (`true`) or remove one row.
    #[allow(dead_code)]
    unsafe fn put_row(&self, man: usize, set: u32, row: u16, v: bool) {
        if (row as usize) < o::ROW_COUNT { self.put_flag(man, set + row as u32, v); }
    }
    #[allow(dead_code)]
    unsafe fn has_row(&self, man: usize, set: u32, row: u16) -> bool { self.get_flag(man, set + row as u32) == Some(true) }

    // --- persisted shop state: slots 0..2 = 3 x 9-bit row indices + valid bit (flags 7800..7827, the original
    // layout); slots 3.. = 9 bits each from STATE_FLAG_EXTRA_BASE holding row index + 1 (0 = empty) ---
    unsafe fn read_state(&self, man: usize) -> Option<Vec<u16>> {
        let mut bits: u32 = 0;
        for b in 0..=o::STATE_FLAG_BITS {
            if self.get_flag(man, o::STATE_FLAG_BASE + b) == Some(true) { bits |= 1 << b; }
        }
        if bits & (1 << o::STATE_FLAG_BITS) == 0 { return None; }
        let mut v = vec![(bits & 0x1FF) as u16, ((bits >> 9) & 0x1FF) as u16, ((bits >> 18) & 0x1FF) as u16];
        for s in 0..(config::MAX_SHOP_SLOTS - 3) {
            let mut x: u32 = 0;
            for b in 0..9 { if self.get_flag(man, o::STATE_FLAG_EXTRA_BASE + s * 9 + b) == Some(true) { x |= 1 << b; } }
            if x != 0 { v.push((x - 1) as u16); }
        }
        Some(v)
    }
    unsafe fn write_state(&self, man: usize, idx: &[u16]) {
        let at = |k: usize| idx.get(k).copied().unwrap_or(0x1FF) as u32 & 0x1FF; // 0x1FF = no row (empty slot)
        for s in 0..(config::MAX_SHOP_SLOTS - 3) {
            let x = idx.get(3 + s as usize).map(|i| *i as u32 + 1).unwrap_or(0);
            for b in 0..9 { self.put_flag(man, o::STATE_FLAG_EXTRA_BASE + s * 9 + b, (x >> b) & 1 != 0); }
        }
        let bits = at(0) | (at(1) << 9) | (at(2) << 18) | (1 << o::STATE_FLAG_BITS);
        for b in 0..=o::STATE_FLAG_BITS {
            if !self.put_flag(man, o::STATE_FLAG_BASE + b, (bits >> b) & 1 != 0) { logf!("put_flag failed for {}", o::STATE_FLAG_BASE + b); }
        }
        let mut back: u32 = 0;
        for b in 0..=o::STATE_FLAG_BITS {
            if self.get_flag(man, o::STATE_FLAG_BASE + b) == Some(true) { back |= 1 << b; }
        }
        if back != bits { logf!("STATE WRITE MISMATCH: wrote {:#X} read {:#X} (flags {}..)", bits, back, o::STATE_FLAG_BASE); }
    }
}

// ---------------------------------------------------------------- param repository
/// Live param rows through the game's own repository ([exe+4E27300], the object the shop builder
/// uses). Verified 2026-10-26: same addresses at title and after Continue, so writes are durable.
unsafe fn param_file(base: usize, idx: usize) -> Option<usize> {
    let repo = mem::rd_usize(base + o::PARAM_REPO_GLOBAL);
    if repo == 0 || !mem::is_readable(repo + o::REPO_SLOT0, o::REPO_SLOTS * o::REPO_SLOT_STRIDE) { return None; }
    let slot = repo + o::REPO_SLOT0 + idx * o::REPO_SLOT_STRIDE;
    if mem::rd_u32(slot) == 0 { return None; }
    let res = mem::rd_usize(slot + 8);
    if res == 0 || !mem::is_readable(res, 0x88) { return None; }
    let holder = mem::rd_usize(res + o::RES_FILE_HOLDER);
    if holder == 0 || !mem::is_readable(holder, 0x88) { return None; }
    let file = mem::rd_usize(holder + o::RES_FILE_HOLDER);
    if file == 0 || !mem::is_readable(file, 0x40) { return None; }
    Some(file)
}

/// Repository index of a param by its name (wide string at res+0x18, MSVC wstring: inline below 8 chars).
unsafe fn param_index(base: usize, name: &str) -> Option<usize> {
    let repo = mem::rd_usize(base + o::PARAM_REPO_GLOBAL);
    if repo == 0 || !mem::is_readable(repo + o::REPO_SLOT0, o::REPO_SLOTS * o::REPO_SLOT_STRIDE) { return None; }
    let want: Vec<u16> = name.encode_utf16().collect();
    (0..o::REPO_SLOTS).find(|&i| {
        let slot = repo + o::REPO_SLOT0 + i * o::REPO_SLOT_STRIDE;
        let res = mem::rd_usize(slot + 8);
        if mem::rd_u32(slot) == 0 || res == 0 || !mem::is_readable(res, 0x38) { return false; }
        let (len, cap) = (mem::rd_usize(res + o::RES_NAME + 0x10), mem::rd_usize(res + o::RES_NAME + 0x18));
        if len != want.len() { return false; }
        let s = if cap >= 8 { mem::rd_usize(res + o::RES_NAME) } else { res + o::RES_NAME };
        mem::is_readable(s, len * 2) && (0..len).all(|k| mem::rd_u16(s + k * 2) == want[k])
    })
}

/// equipIds of the tank legs: EquipParamProtector rows with legsType 300..302 (Paramdex LEGS_TYPE).
unsafe fn tank_legs(base: usize) -> Vec<u32> {
    let Some(f) = param_index(base, "EquipParamProtector").and_then(|i| param_file(base, i)) else {
        log("EquipParamProtector not found: no tank legs known"); return Vec::new();
    };
    param_rows(f).into_iter()
        .filter(|&(id, r)| id / 1_000_000 == 53 && (300..=302).contains(&mem::rd_u16(r + o::PROTECTOR_LEGS_TYPE)))
        .map(|(id, _)| id).collect()
}

/// Every (row id, row address) of a param file.
unsafe fn param_rows(file: usize) -> Vec<(u32, usize)> {
    let n = mem::rd_u16(file + o::PARAMFILE_ROWCOUNT) as usize;
    if !mem::is_readable(file + o::PARAMFILE_ROWS, n * o::PARAMFILE_ROW_STRIDE) { return Vec::new(); }
    (0..n).map(|i| {
        let e = file + o::PARAMFILE_ROWS + i * o::PARAMFILE_ROW_STRIDE;
        (mem::rd_u32(e), file + mem::rd_usize(e + 8))
    }).collect()
}

/// Apply the economy to the live params, once at boot: the game loads its params once per start (no reload in a
/// mission, a return to the title or a Continue; RE_NOTES "Param edits applied once"). Idempotent; returns how many
/// fields it changed. Values this function wrote, by address: a field that already holds our value is not scaled
/// twice.
static ECONOMY_WRITTEN: std::sync::Mutex<Option<std::collections::HashMap<usize, i32>>> = std::sync::Mutex::new(None);

unsafe fn apply_economy(base: usize) -> usize {
    let cfg = config::get();
    let mut guard = ECONOMY_WRITTEN.lock().unwrap();
    let written = guard.get_or_insert_with(Default::default);
    let mut changed = 0usize;
    // integer pay fields, or (float) f32 cost scales stored by their bits
    let mut scale = |a: usize, pct: u32, float: bool| {
        let v = mem::rd_i32(a);
        if (!float && v <= 0) || written.get(&a) == Some(&v) { return; }
        let nv = if float { (f32::from_bits(v as u32) * pct as f32 / 100.0).to_bits() as i32 } else { (v as i64 * pct as i64 / 100) as i32 };
        if nv != v { mem::wr_i32(a, nv); changed += 1; }
        written.insert(a, nv);
    };
    if let Some(f) = param_file(base, o::PARAM_IDX_MISSION) {
        for (_, r) in param_rows(f) {
            if !mem::is_readable(r, o::MISSION_ROW_SIZE) { continue; }
            scale(r + o::MISSION_CURRENCY_REWARD, cfg.mission_pay_percent, false);
            for k in 0..o::MISSION_EVENT_REWARDS { scale(r + o::MISSION_EVENT_REWARD0 + k * 8, cfg.mission_pay_percent, false); }
            for k in 0..o::MISSION_ENEMY_REWARDS { scale(r + o::MISSION_ENEMY_REWARD0 + k * 8, cfg.mission_pay_percent, false); }
            scale(r + o::MISSION_BULLET_COST_RATE, cfg.ammo_cost_percent, true);
            scale(r + o::MISSION_REPAIR_COST_COEF, cfg.repair_cost_percent, true);
        }
    }
    if let Some(f) = param_file(base, o::PARAM_IDX_ARENA) {
        for (_, r) in param_rows(f) {
            if mem::is_readable(r, o::ARENA_ROW_SIZE) { scale(r + o::ARENA_REPEAT_COAM, cfg.arena_repeat_pay_percent, false); }
        }
    }
    // the first mission of every run (New Game always starts with it, Replay Mission is locked) pays the starting
    // credits: shown on its results screen, and the game nets that mission's costs from it
    if let Some(r) = param_file(base, o::PARAM_IDX_MISSION)
        .and_then(|f| param_rows(f).into_iter().find(|(id, _)| *id == o::MISSION_FIRST).map(|(_, r)| r))
    {
        let (a, v) = (r + o::MISSION_CURRENCY_REWARD, cfg.starting_credits as i32);
        if mem::rd_i32(a) != v { mem::wr_i32(a, v); changed += 1; }
        written.insert(a, v);
    }
    changed
}

// ---------------------------------------------------------------- pause menu block
static PAUSE_BLOCKED: AtomicUsize = AtomicUsize::new(0);
type FnRequestPause = unsafe extern "system" fn(this: usize);

/// Replaces the game step's call to RequestPauseMenu: dropped while a mission is active (rule: no pause
/// menu, no settings, no retry/quit during missions), forwarded otherwise.
unsafe extern "system" fn pause_hook(this: usize) {
    let base = mem::module_base();
    let game = Game::new(base);
    if let Some(gd) = game.game_data() {
        if game.defer_byte(gd) == 1 { PAUSE_BLOCKED.fetch_add(1, Relaxed); return; }
    }
    let orig: FnRequestPause = core::mem::transmute(base + o::FN_REQUEST_PAUSE_MENU);
    orig(this);
}

// ---------------------------------------------------------------- shop predicate hook
/// The chosen shop rows as their `equipId` (row +0). 0 = empty slot.
static CHOSEN: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];
static PRED_CALLS: AtomicUsize = AtomicUsize::new(0);
/// DEBUG: item id forced as the next sortie's reward (0 = none).
static FORCE_REWARD: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());
/// FORCE_REWARD tag: the low 16 bits are an OsReinforce row (debug `force_os`)
const FORCE_OS_TAG: u32 = 0x8000_0000;
static PAGE_CACHE_MAN: AtomicUsize = AtomicUsize::new(0);
static PAGE_CACHE: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];
fn clear_page_cache() { for c in &PAGE_CACHE { c.store(0, Relaxed); } }

/// Replacement for the release predicate at the shop builder's call site. rcx = itemCtx.
/// Must mirror the original's contract: returns al != 0 to list the row.
unsafe extern "system" fn shop_pred(item_ctx: *const u8) -> u8 {
    PRED_CALLS.fetch_add(1, Relaxed);
    if item_ctx.is_null() { return 0; }
    let row = mem::rd_usize(item_ctx as usize + o::ITEMCTX_ROW);
    if row == 0 { return 0; }
    let id = mem::rd_u32(row + o::ROW_EQUIP_ID);
    if id == 0 { return 0; }
    for c in &CHOSEN {
        if c.load(Relaxed) == id { return 1; }
    }
    // buy-back rows, and the starter frame parts / internals: always for sale (the ownership filter hides owned ones)
    (buyback::offered(row) || penalty::FRAME_SLOTS.iter().any(|s| s.2 == id)) as u8
}

fn set_chosen(ids: &[u32]) {
    for (k, c) in CHOSEN.iter().enumerate() { c.store(ids.get(k).copied().unwrap_or(0), Relaxed); }
}
fn chosen() -> Vec<u32> { CHOSEN.iter().map(|c| c.load(Relaxed)).collect() }

/// Patch `call exe+159ACB0` at exe+E89074 into `call cave`, cave = `mov rax, shop_pred; jmp rax`.
/// The cave must sit within +-2 GB of the call site, so it is allocated just below the module.
/// A fresh RWX page within +-2 GB below the image (for 5-byte rel32 jumps/calls).
unsafe fn alloc_near_page(base: usize) -> Option<usize> {
    // the block reserved at load time first: by the time the hooks are installed the game's heaps can fill every
    // 64 KB slot within rel32 reach of the image (seen: the DLL aborted at boot on some launches)
    let used = POOL_USED.fetch_add(0x1000, Relaxed);
    if POOL.load(Relaxed) != 0 && used + 0x1000 <= POOL_SIZE { return Some(POOL.load(Relaxed) + used); }
    alloc_near(base, 0x1000)
}

/// Hook memory reserved next to the image when the DLL is loaded, carved into pages by `alloc_near_page`.
static POOL: AtomicUsize = AtomicUsize::new(0);
static POOL_USED: AtomicUsize = AtomicUsize::new(0);
const POOL_SIZE: usize = 0x10_0000;

/// `size` bytes of executable memory within rel32 reach of the image (below it first, then above).
unsafe fn alloc_near(base: usize, size: usize) -> Option<usize> {
    let mut hint = base - ((size + 0xFFFF) & !0xFFFF);
    while hint > base.saturating_sub(0x7000_0000) {
        let p = VirtualAlloc(hint as *mut c_void, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        if !p.is_null() { return Some(p as usize); }
        hint -= 0x10000;
    }
    // below the image is full (every page takes a 64 KB allocation slot): above it, still within rel32 reach
    let mut hint = (base + o::IMAGE_SIZE + 0xFFFF) & !0xFFFF;
    while hint < base + 0x7000_0000 - size {
        let p = VirtualAlloc(hint as *mut c_void, size, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        if !p.is_null() { return Some(p as usize); }
        hint += 0x10000;
    }
    None
}

/// `len` bytes of executable memory within rel32 reach of the image, carved from shared near pages (one page holds
/// many call-hook stubs instead of one 64 KB allocation slot each).
unsafe fn alloc_cave(base: usize, len: usize) -> Option<usize> {
    static PAGE: AtomicUsize = AtomicUsize::new(0);
    static USED: AtomicUsize = AtomicUsize::new(0);
    let len = (len + 15) & !15;
    if PAGE.load(Relaxed) == 0 || USED.load(Relaxed) + len > 0x1000 {
        PAGE.store(alloc_near_page(base)?, Relaxed);
        USED.store(0, Relaxed);
    }
    Some(PAGE.load(Relaxed) + USED.fetch_add(len, Relaxed))
}

unsafe fn write_code(site: usize, bytes: &[u8]) -> bool {
    let mut old: u32 = 0;
    if VirtualProtect(site as *mut c_void, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) == 0 { return false; }
    core::ptr::copy_nonoverlapping(bytes.as_ptr(), site as *mut u8, bytes.len());
    VirtualProtect(site as *mut c_void, bytes.len(), old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), site as *const c_void, bytes.len());
    true
}

/// Where the COAM detour stores its observations (inside the cave page).
static COAM_LAST_RAW: AtomicUsize = AtomicUsize::new(0); // address of i32: unclamped (old + delta) of the last AddCoam
static COAM_CALLS: AtomicUsize = AtomicUsize::new(0);    // address of u32: number of AddCoam calls

/// Detour AddCoam (exe+48DD70: `mov r8d,[rcx+74]; lea r9d,[r8+rdx]` = 8 bytes) through a cave
/// that re-executes those two instructions, records r9d (the UNCLAMPED new balance) and a call
/// counter, then jumps back. No call into Rust from the game thread: just two stores.
unsafe fn install_coam_hook(base: usize) -> Result<usize, String> {
    let site = base + o::FN_ADD_COAM;
    let expect: [u8; 8] = [0x44, 0x8B, 0x41, 0x74, 0x45, 0x8D, 0x0C, 0x10];
    let mut cur = [0u8; 8];
    core::ptr::copy_nonoverlapping(site as *const u8, cur.as_mut_ptr(), 8);
    if cur != expect { return Err(format!("unexpected bytes at AddCoam: {:02X?}", cur)); }
    let cave = alloc_near_page(base).ok_or("no near page for the COAM cave")?;
    let data_raw = cave + 0x100;
    let data_cnt = cave + 0x104;
    let mut c: Vec<u8> = Vec::new();
    c.extend_from_slice(&expect);                                   // original 2 instructions
    // mov [rip+d32], r9d  (44 89 0D d32)
    let at = cave + c.len(); c.extend_from_slice(&[0x44, 0x89, 0x0D]);
    c.extend_from_slice(&((data_raw as i64 - (at as i64 + 7)) as i32).to_le_bytes());
    // lock inc dword [rip+d32]  (F0 FF 05 d32)
    let at = cave + c.len(); c.extend_from_slice(&[0xF0, 0xFF, 0x05]);
    c.extend_from_slice(&((data_cnt as i64 - (at as i64 + 7)) as i32).to_le_bytes());
    // jmp site+8  (E9 rel32)
    let at = cave + c.len(); c.push(0xE9);
    c.extend_from_slice(&(((site + 8) as i64 - (at as i64 + 5)) as i32).to_le_bytes());
    core::ptr::copy_nonoverlapping(c.as_ptr(), cave as *mut u8, c.len());
    COAM_LAST_RAW.store(data_raw, Relaxed);
    COAM_CALLS.store(data_cnt, Relaxed);
    // site: jmp cave ; nop nop nop
    let mut patch = [0x90u8; 8];
    patch[0] = 0xE9;
    patch[1..5].copy_from_slice(&((cave as i64 - (site as i64 + 5)) as i32).to_le_bytes());
    if !write_code(site, &patch) { return Err("VirtualProtect failed on AddCoam".into()); }
    Ok(cave)
}

/// Replace a 5-byte `call target` at `site_rva` with `call cave`, cave = `mov rax, f; jmp rax`.
unsafe fn install_call_hook(base: usize, site_rva: usize, target_rva: usize, f: usize) -> Result<usize, String> {
    let site = base + site_rva;
    let expect_rel = (target_rva as i64 - (site_rva as i64 + 5)) as i32;
    if mem::rd_u8(site) != 0xE8 || mem::rd_i32(site + 1) != expect_rel {
        return Err(format!("unexpected bytes at exe+{:X}: {:02X} rel={:#X}", site_rva, mem::rd_u8(site), mem::rd_i32(site + 1)));
    }
    let cave = alloc_cave(base, 12).ok_or("no near page for the cave")?;
    let mut code = [0u8; 12];
    code[0] = 0x48; code[1] = 0xB8;
    code[2..10].copy_from_slice(&(f as u64).to_le_bytes());
    code[10] = 0xFF; code[11] = 0xE0;
    core::ptr::copy_nonoverlapping(code.as_ptr(), cave as *mut u8, code.len());
    let mut patch = [0u8; 5];
    patch[0] = 0xE8;
    patch[1..5].copy_from_slice(&((cave as i64 - (site as i64 + 5)) as i32).to_le_bytes());
    if !write_code(site, &patch) { return Err("VirtualProtect failed".into()); }
    Ok(cave)
}

unsafe fn install_shop_hook(base: usize) -> Result<usize, String> {
    let site = base + o::SHOP_PRED_CALLSITE;
    let expect_rel = (o::FN_SHOP_PRED as i64 - (o::SHOP_PRED_CALLSITE as i64 + 5)) as i32;
    if mem::rd_u8(site) != 0xE8 || mem::rd_i32(site + 1) != expect_rel {
        return Err(format!("unexpected bytes at call site: {:02X} rel={:#X}", mem::rd_u8(site), mem::rd_i32(site + 1)));
    }
    let cave = alloc_cave(base, 12).ok_or("no near page for the cave")?;
    let target = shop_pred as *const () as usize as u64;
    let mut code = [0u8; 12];
    code[0] = 0x48; code[1] = 0xB8; // mov rax, imm64
    code[2..10].copy_from_slice(&target.to_le_bytes());
    code[10] = 0xFF; code[11] = 0xE0; // jmp rax
    core::ptr::copy_nonoverlapping(code.as_ptr(), cave as *mut u8, code.len());

    let rel = (cave as i64 - (site as i64 + 5)) as i32;
    let mut old: u32 = 0;
    if VirtualProtect(site as *mut c_void, 5, PAGE_EXECUTE_READWRITE, &mut old) == 0 {
        return Err("VirtualProtect failed on the call site".into());
    }
    let mut patch = [0u8; 5];
    patch[0] = 0xE8;
    patch[1..5].copy_from_slice(&rel.to_le_bytes());
    core::ptr::copy_nonoverlapping(patch.as_ptr(), site as *mut u8, 5);
    VirtualProtect(site as *mut c_void, 5, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), site as *const c_void, 5);
    Ok(cave)
}

// ---------------------------------------------------------------- run logic
struct Run {
    game: Game,
    pool: Vec<Part>,
    rng: Rng,
    /// gaitem handles of the rewards we queued for the current mission
    reward_handles: Vec<u32>,
    /// the parts chosen for the current sortie: a retry re-queues THESE parts (no reroll by retrying)
    reward_parts: Vec<Part>,
    /// reward choice: the current sortie's offers, by reward (reward_parts = all of them)
    reward_sets: Vec<Vec<Part>>,
    last_grant_tick: u64,
    /// a mission actually ran (defer went 0->1) since the last reroll; guards against a stale
    /// 5074 read on save load triggering a reroll
    mission_ran: bool,
    /// a mission was cleared while the Parts Shop was still locked: roll the first shop when it unlocks
    market_wait: bool,
}

impl Run {
    fn part_by_idx(&self, idx: u16) -> Option<&Part> { self.pool.iter().find(|p| p.idx == idx) }
    /// The shop slots' parts (None = empty slot, 0x1FF in the state).
    fn slot_parts(&self, idx: &[u16]) -> Vec<Option<Part>> { idx.iter().map(|i| self.part_by_idx(*i).copied()).collect() }

    /// Parts that can roll: the shop's weapons (`Some(true)`) or other parts (`Some(false)`), or both (`None`,
    /// rewards), not in `exclude`. Weapons and frame parts roll even when owned (each copy is its own inventory
    /// entry); unique parts not while owned. Never again: a part in ROWS_LOST (lost forever, or offered its
    /// [max_occurrences] times), an OS Tuning with its OS_GONE bit; nor a part whose offer count already reached its
    /// max (a max lowered mid-run). OS Tunings: rewards only (never in the shop), while below
    /// their max level. Each lineup row is its own part (left and right weapon versions included). Parts out of
    /// the pools (`= 0`, weight 0) never roll.
    unsafe fn candidates(&self, gd: usize, man: usize, owned: &[u32], weapon: Option<bool>, exclude: &[u16]) -> Vec<Part> {
        let lost = self.game.read_rows(man, o::ROWS_LOST);
        let counts = occur::read(&self.game, man);
        self.pool.iter().copied().filter(|p| p.in_pool() && weapon.map_or(true, |w| !p.is_os() && p.is_weapon() == w)
            && !exclude.contains(&p.idx)
            && !(p.max > 0 && occur::count(&counts, p) >= p.max as u32)
            && !(p.unique() && owned.contains(&p.grant_id()))
            && !(if p.is_os() { os::gone(&self.game, man, p.os) } else { lost.contains(&p.idx) })
            && !(p.is_os() && !os::tuning(p.os).zip(os::level(gd, p.os)).is_some_and(|(t, l)| l < t.max))).collect()
    }

    /// config shop_weapons weapons + shop_parts other parts, by rarity, all different. The parts being awarded right
    /// now are left out: the shop is rolled at the mission clear, before the commit inserts them, so their copies
    /// would be missing from the slot's baseline (dupshop.rs).
    unsafe fn pick_shop(&mut self, gd: usize, man: usize) -> Vec<Part> {
        let cfg = config::get();
        let owned = self.game.owned_ids(gd);
        let mut taken: Vec<u16> = self.reward_parts.iter().map(|p| p.idx).collect();
        let mut out = Vec::new();
        // shop size after the chips' shop.weapons / shop.parts modifiers, MAX_SHOP_SLOTS at most
        let weapons = chip::modify_count(chip::Target::ShopWeapons, cfg.shop_weapons, 0, config::MAX_SHOP_SLOTS);
        let parts = chip::modify_count(chip::Target::ShopParts, cfg.shop_parts, 0, config::MAX_SHOP_SLOTS - weapons);
        for (count, weapon) in [(weapons, true), (parts, false)] {
            for n in 0..count {
                let cands = self.candidates(gd, man, &owned, Some(weapon), &taken);
                let Some(p) = pick_weighted(&mut self.rng, &cands) else {
                    logf!("shop: no {} left, {} slot(s) empty", if weapon { "weapon" } else { "part" }, count - n);
                    break;
                };
                taken.push(p.idx);
                out.push(p);
            }
        }
        out
    }

    /// Reward candidates, minus `taken` and the unique parts of the current shop (given, one would leave its slot
    /// empty).
    unsafe fn reward_candidates(&self, gd: usize, man: usize, taken: &[u16]) -> Vec<Part> {
        let owned = self.game.owned_ids(gd);
        let mut exclude = taken.to_vec();
        exclude.extend(self.game.read_state(man).unwrap_or_default().into_iter().filter(|i| self.part_by_idx(*i).is_some_and(|p| p.unique())));
        self.candidates(gd, man, &owned, None, &exclude)
    }

    /// [max_occurrences]: these parts were offered (reward grid at the clear, shop at the reroll): one more offer each,
    /// never again once at their max.
    unsafe fn mark_offered(&self, man: usize, parts: &[Part], why: &str) {
        let counted: Vec<String> = parts.iter().filter(|p| p.max > 0).map(|p| {
            let n = occur::record(&self.game, man, p);
            format!("{} {}/{}{}", p.name(), n, p.max, if n >= p.max as u32 { " (never again)" } else { "" })
        }).collect();
        if !counted.is_empty() { logf!("max occurrences, offered ({}): {}", why, counted.join(", ")); }
    }

    unsafe fn apply_state(&self, man: usize, idx: &[u16], why: &str) {
        let ids: Vec<u32> = idx.iter().map(|i| self.part_by_idx(*i).map(|p| p.equip_id).unwrap_or(0)).collect();
        set_chosen(&ids);
        dupshop::load(&self.game, man, &self.slot_parts(idx));
        let pools: Vec<String> = idx.iter().map(|i| self.part_by_idx(*i).map(|p| p.pool_name()).unwrap_or_default()).collect();
        logf!("shop <- rows {:?} = equip {:?} pools {:?} ({})", idx, ids, pools, why);
    }

    /// Queue the mission reward. Called at mission start and again if the queue was cleared
    /// (mission restart from the pause menu empties it while the mission stays active).
    unsafe fn queue_reward(&mut self, gd: usize, man: usize, why: &str) {
        self.last_grant_tick = tick_count();
        if self.reward_parts.is_empty() { self.roll_rewards(gd, man, why); self.force_reward(why); }
        self.reward_handles.clear();
        let owned = self.game.owned_ids(gd);
        let mut offers = Vec::new();
        for p in self.reward_parts.clone() {
            // an OS Tuning queues its core expansion item when not owned yet (kept if picked), else a carrier item
            let keep = p.is_os() && p.equip_id != 0 && !owned.contains(&p.equip_id);
            let id = if p.is_os() && !keep { o::OS_CARRIER_ITEM } else { p.grant_id() };
            let before = self.game.pending(gd);
            let (r, err) = self.game.give_deferred(gd, id);
            let after = self.game.pending(gd);
            let new_handle = after.iter().copied().find(|h| !before.contains(h));
            if let Some(h) = new_handle { self.reward_handles.push(h); }
            let label = if p.is_os() { Some(os::label(gd, p.os)) } else { None };
            offers.push(choice::Offer { id, handle: new_handle.unwrap_or(0), os: p.os, keep, label, pool: p.pool_label() });
            logf!("{}: reward {} {} pool {} item {:#X} -> rax={} err={} handle={:?} pending={}",
                why, if p.is_os() { format!("OS {}", p.os) } else { format!("row {}", p.idx) }, p.name(), p.pool_name(), id, r, err, new_handle.map(|h| format!("{:#X}", h)), after.len());
        }
        // the grid also shows the OS Tunings of plain rewards (their carrier items must leave the list)
        let sets: Vec<usize> = if !self.reward_sets.is_empty() { self.reward_sets.iter().map(|s| s.len()).collect() }
            else if self.reward_parts.iter().any(|p| p.is_os()) { vec![1; offers.len()] } else { Vec::new() };
        if !sets.is_empty() {
            let mut it = offers.into_iter();
            choice::arm(sets.iter().map(|n| it.by_ref().take(*n).collect()).collect());
        }
    }

    /// DEBUG `force_reward <hex item ids>`: the next sortie's rewards (or first offer of each choice) are these parts,
    /// owned or not (duplicate tests). `force_os <rows>` (FORCE_OS_TAG | row): these OS Tunings.
    fn force_reward(&mut self, why: &str) {
        let ids = FORCE_REWARD.lock().map(|mut f| core::mem::take(&mut *f)).unwrap_or_default();
        for (k, id) in ids.into_iter().enumerate() {
            let found = if id & FORCE_OS_TAG != 0 { self.pool.iter().copied().find(|p| p.is_os() && p.os as u32 == id & 0xFFFF) }
                else { self.pool.iter().copied().find(|p| !p.is_os() && p.grant_id() == id) };
            let Some(p) = found else { logf!("{}: forced reward {:#X} not in the pool", why, id); continue };
            if !self.reward_sets.is_empty() {
                let Some(s) = self.reward_sets.get_mut(k).filter(|s| !s.is_empty()) else { break };
                s[0] = p;
                let at: usize = self.reward_sets[..k].iter().map(|s| s.len()).sum();
                self.reward_parts[at] = p;
            } else if k < self.reward_parts.len() { self.reward_parts[k] = p; } else { self.reward_parts.push(p); }
            logf!("{}: DEBUG forced reward {}: {} {} ({:#X})", why, k, p.idx, p.name(), id);
        }
    }

    /// Roll this sortie's parts_per_mission rewards. Reward choice: each reward is an offer of reward_choices parts
    /// (choice.rs).
    unsafe fn roll_rewards(&mut self, gd: usize, man: usize, why: &str) {
        let cfg = config::get();
        // rewards per mission (0..3) and parts per reward (1..12) after the chips' reward.count / reward.choices
        let count = chip::modify_count(chip::Target::RewardCount, cfg.parts_per_mission, 0, 3);
        let choices = chip::modify_count(chip::Target::RewardChoices, cfg.reward_choices, 1, 12);
        if choices > 1 {
            let cands = self.reward_candidates(gd, man, &[]);
            self.reward_sets = choice::roll(&self.game, man, &cands, count, choices as usize);
            self.reward_parts = self.reward_sets.iter().flatten().copied().collect();
            logf!("{}: reward offers {:?}", why, self.reward_sets.iter().map(|s| s.iter().map(|p| p.idx).collect::<Vec<_>>()).collect::<Vec<_>>());
            return;
        }
        for _ in 0..count {
            let taken: Vec<u16> = self.reward_parts.iter().map(|p| p.idx).collect();
            let cands = self.reward_candidates(gd, man, &taken);
            match pick_weighted(&mut self.rng, &cands) {
                Some(p) => self.reward_parts.push(p),
                None => { log("reward: nothing to give"); break; }
            }
        }
    }

    /// While a mission is active: if our queued rewards disappeared (queue cleared), re-queue them.
    unsafe fn check_reward(&mut self, gd: usize, man: usize) {
        if self.reward_handles.is_empty() { return; }
        let pending = self.game.pending(gd);
        if self.reward_handles.iter().any(|h| !pending.contains(h)) && tick_count().saturating_sub(self.last_grant_tick) > 2000 {
            logf!("reward handles {:X?} vanished from the queue (mission restart?)", self.reward_handles);
            self.queue_reward(gd, man, "re-queue");
        }
    }

    /// The garage just opened (save loaded, or back from a mission). A set escrow flag means the last sortie
    /// was not cleared: death, quit or the game killed mid-mission.
    unsafe fn on_garage_open(&mut self, man: usize, why: &str) {
        event::emit(event::Event::GarageOpened);
        if self.game.get_flag(man, o::FLAG_ESCROW) == Some(true) {
            logf!("garage opened ({}): escrow flag set -> PENALTY queued for the game thread", why);
            penalty::PENALTY_REQUEST.store(true, Relaxed);
        } else {
            logf!("garage opened ({}): escrow clear, no penalty", why);
        }
    }

    /// A mission was cleared: reroll the shop every reroll_interval missions (counter in the save). The first
    /// mission end of a run with the Parts Shop open (no shop state yet) always rolls; while the shop is locked
    /// nothing happens (no roll, no count) and the first shop is rolled when it unlocks.
    unsafe fn on_mission_end(&mut self, gd: usize, man: usize) {
        let cfg = config::get();
        if self.game.get_flag(man, o::FLAG_MARKET) != Some(true) {
            self.market_wait = true;
            log("mission end: Parts Shop still locked, no shop yet");
            return;
        }
        self.market_wait = false;
        let first = self.game.read_state(man).is_none();
        let done = self.game.get_bits(man, o::REROLL_COUNTER_BASE, o::REROLL_COUNTER_BITS) + 1;
        if !first && done < cfg.reroll_interval {
            self.game.put_bits(man, o::REROLL_COUNTER_BASE, o::REROLL_COUNTER_BITS, done);
            logf!("mission end: shop kept ({}/{} missions before the reroll)", done, cfg.reroll_interval);
            return;
        }
        self.game.put_bits(man, o::REROLL_COUNTER_BASE, o::REROLL_COUNTER_BITS, 0);
        self.reroll(gd, man);
    }

    unsafe fn reroll(&mut self, gd: usize, man: usize) {
        event::emit(event::Event::ShopReroll);
        let sel = self.pick_shop(gd, man);
        if sel.is_empty() { log("mission end: nothing left to offer"); }
        let idx: Vec<u16> = sel.iter().map(|p| p.idx).collect();
        self.mark_offered(man, &sel, "shop");
        self.game.write_state(man, &idx);
        dupshop::write(&self.game, man, &self.slot_parts(&idx));
        buyback::clear(&self.game, man);
        self.apply_state(man, &idx, "reroll at mission end");
    }
}

// ---------------------------------------------------------------- boot
fn boot() {
    let _ = std::fs::remove_file(log_path());
    log("=== ac6_roguelike DLL boot ===");
    let base = mem::module_base();
    logf!("module base = {:#X}, hook memory reserved at load: {:#X}", base, POOL.load(Relaxed));
    config::load();
    chip::load();
    msg::set_base(base);
    // before the menus are preloaded: our reward choice movie takes a spare menu table entry
    for _ in 0..100 { if unsafe { mem::rd_usize(base + o::MENU_TABLE_SPARE_ENTRY) } != 0 { break; } sleep_ms(20); }
    unsafe { rewardmenu::register_movie(base); cap::register_movie(base); }
    let game = unsafe { Game::new(base) };

    // wait for GameDataMan (title screen), up to ~60 s
    let mut gd_seen = false;
    for _ in 0..600 {
        if unsafe { game.game_data() }.is_some() { gd_seen = true; break; }
        sleep_ms(100);
    }
    if !gd_seen { log("GameDataMan never appeared (timeout) — aborting boot"); return; }

    // pool from the live EquipmentLineupParam through the param repository (retry until it is built)
    let mut pool = Vec::new();
    for _ in 0..40 {
        if let Some(f) = unsafe { param_file(base, o::PARAM_IDX_LINEUP) } {
            let rows = unsafe { param_rows(f) };
            if rows.len() == o::ROW_COUNT {
                let lu = Lineup { data: rows[0].1 };
                if unsafe { anchors_ok(lu.data) } {
                    unsafe { release_rank_rewards(&lu); }
                    if !unsafe { os::init(base) } { sleep_ms(500); continue; }
                    pool = unsafe { build_pool(&lu) };
                    unsafe { penalty::init(base, &lu, tank_legs(base)); buyback::init(base, &lu); }
                    logf!("lineup (repository) at {:#X}: pool = {} rows ({} weapons, {} OS Tunings)", lu.data, pool.len(),
                        pool.iter().filter(|p| p.is_weapon()).count(), pool.iter().filter(|p| p.is_os()).count());
                    if !pool.is_empty() { break; }
                }
            }
        }
        sleep_ms(500);
    }
    if pool.is_empty() { log("EquipmentLineupParam not found — aborting boot"); return; }

    if !ENABLE_PATCHES { log("ENABLE_PATCHES=false: read-only, done"); return; }

    match unsafe { install_shop_hook(base) } {
        Ok(cave) => logf!("shop predicate hook installed: exe+{:X} -> cave {:#X} -> shop_pred {:#X}", o::SHOP_PRED_CALLSITE, cave, shop_pred as *const () as usize),
        Err(e) => { logf!("shop hook FAILED: {} — aborting", e); return; }
    }

    match unsafe { penalty::install(base) } {
        Ok(cave) => logf!("game-thread hook installed: exe+{:X} -> cave {:#X}", o::FRAME_HOOK_CALLSITE, cave),
        Err(e) => logf!("game-thread hook FAILED: {} (penalty disabled)", e),
    }
    match unsafe { install_call_hook(base, o::PAUSE_REQ_CALLSITE, o::FN_REQUEST_PAUSE_MENU, pause_hook as *const () as usize) } {
        Ok(cave) => logf!("pause hook installed: exe+{:X} -> cave {:#X}", o::PAUSE_REQ_CALLSITE, cave),
        Err(e) => logf!("pause hook FAILED: {}", e),
    }
    match unsafe { install_call_hook(base, o::DEATH_MENU_CALLSITE, o::FN_OPEN_RETRY_MENU, penalty::death_menu_hook as *const () as usize) } {
        Ok(cave) => logf!("death-menu hook installed: exe+{:X} -> cave {:#X}", o::DEATH_MENU_CALLSITE, cave),
        Err(e) => logf!("death-menu hook FAILED: {}", e),
    }
    for site in o::SELL_PRICE_CALLSITES {
        match unsafe { install_call_hook(base, site, o::FN_SELL_PRICE, penalty::sell_price_hook as *const () as usize) } {
            Ok(cave) => logf!("sell price hook installed: exe+{:X} -> cave {:#X}", site, cave),
            Err(e) => logf!("sell price hook exe+{:X} FAILED: {}", site, e),
        }
    }
    for site in o::SALE_REMOVE_CALLSITES {
        match unsafe { install_call_hook(base, site, o::FN_SHOP_REMOVE, buyback::sale_hook as *const () as usize) } {
            Ok(cave) => logf!("sale hook installed: exe+{:X} -> cave {:#X}", site, cave),
            Err(e) => logf!("sale hook exe+{:X} FAILED: {} (buy-back misses Vente sales)", site, e),
        }
    }
    match unsafe { install_call_hook(base, o::OPEN_SESSION_CALLSITE, o::FN_OPEN_SESSION, penalty::open_session_hook as *const () as usize) } {
        Ok(cave) => logf!("open-session hook installed: exe+{:X} -> cave {:#X}", o::OPEN_SESSION_CALLSITE, cave),
        Err(e) => logf!("open-session hook FAILED: {}", e),
    }
    unsafe { rewardmenu::install(base); chip::install(base); event::install(base); chip::init_ammo(base); }
    unsafe { dupshop::install(base); }
    unsafe { cap::install(base); }
    match unsafe { msg::install(base) } {
        Ok(()) => logf!("message lookup hook installed: exe+{:X}", o::FN_MSG_LOOKUP),
        Err(e) => logf!("message lookup hook FAILED: {}", e),
    }
    match unsafe { install_coam_hook(base) } {
        Ok(cave) => logf!("COAM hook installed: exe+{:X} -> cave {:#X}", o::FN_ADD_COAM, cave),
        Err(e) => logf!("COAM hook FAILED: {} (game-over detection disabled)", e),
    }
    let n = unsafe { apply_economy(base) };
    logf!("economy: {} reward fields scaled at boot", n);
    if config::get().os_rewards && unsafe { os::lock_menu(base) } { log("OS Tuning menu locked (OS Tunings are rewards)"); }
    if config::get().os_rewards && unsafe { os::cut_allmind_talk(base) } { log("ALLMIND OS Tuning lines silenced"); }
    // RE: %USERPROFILE%/ac6_watch.txt = trace commands run at every boot (kept, unlike ac6_debug.txt)
    let watch = std::path::Path::new(&std::env::var("USERPROFILE").unwrap_or_default()).join("ac6_watch.txt");
    for l in std::fs::read_to_string(&watch).unwrap_or_default().lines().filter(|l| l.starts_with("watch ")) { unsafe { trace::command(l.trim()); } }

    let mut run = Run { game, pool, rng: Rng::seeded(), reward_handles: Vec::new(), reward_parts: Vec::new(), reward_sets: Vec::new(), last_grant_tick: 0, mission_ran: false, market_wait: false };
    let mut prev_defer: Option<u8> = None;
    let mut coam_seen: u32 = 0;
    // death tax state for the current sortie
    let mut alive = false;        // AP > 0 seen since the last death / sortie start
    let mut alive_chr = 0usize;   // character object that was last seen alive (a zero must come from it)
    let mut dead = false;         // died and not respawned yet
    let mut cleared_seen = false; // 5074 rose during this sortie
    let mut prev_loaded = false;
    let mut title_at: u64 = 0;
    let mut exit_phase = 0u8;   // game over exit: 1 close the session, 2 wait for its save, 3 leaving for the title
    let mut escrow_save_pending = false;
    let mut prev_talk = false;
    let mut coam_last_raw: i32 = i32::MIN;
    let mut coam_last_neg: i32 = 0;
    let mut coam_last_bal: i32 = i32::MIN;
    let mut prev_clear: Option<u8> = None; // flag 5074: mission cleared, results pending
    let mut prev_post = false; // flag 3139: post-mission garage sequence (set after the commit, cleared when the garage menu is back)
    let mut prev_state: Option<Option<Vec<u16>>> = None;
    let mut prev_gd = 0usize;
    let mut tick: u64 = 0;
    loop {
        tick += 1;
        unsafe {
            let gd = match run.game.game_data() { Some(g) => g, None => { prev_defer = None; sleep_ms(250); continue; } };
            if gd != prev_gd { logf!("GameData = {:#X}", gd); prev_gd = gd; prev_defer = None; clear_page_cache(); }
            if tick % 50 == 0 { clear_page_cache(); } // re-walk the flag map every 5 s (pages can be reallocated)

            // (1) persisted shop state (flags) -> chosen ids. Handles Continue / New Game / our own writes.
            if let Some(man) = run.game.flag_man() {
                let st = run.game.read_state(man);
                if prev_state.as_ref() != Some(&st) {
                    match &st {
                        Some(idx) => run.apply_state(man, idx, "state read from flags"),
                        None => { set_chosen(&[]); log("no shop state in flags: shop empty until the first mission end"); }
                    }
                    prev_state = Some(st);
                }

                // COAM: observe every AddCoam through the detour
                let cnt_at = COAM_CALLS.load(Relaxed);
                if cnt_at != 0 {
                    let cnt = mem::rd_u32(cnt_at);
                    if cnt != coam_seen {
                        // the HUD calls AddCoam(0) ~100x/s: only report real changes
                        let raw = mem::rd_i32(COAM_LAST_RAW.load(Relaxed));
                        let now = mem::rd_i32(gd + o::GD_COAM);
                        if raw != coam_last_raw || now != coam_last_bal {
                            logf!("COAM: {} -> {} (unclamped {})", coam_last_bal, now, raw);
                            coam_last_raw = raw; coam_last_bal = now;
                        }
                        if raw < 0 && raw != coam_last_neg {
                            logf!("balance would be {} (< 0): auto-sell queued", raw);
                            penalty::AUTOSELL_DEFICIT.store(raw, Relaxed);
                        }
                        coam_last_neg = raw;
                        coam_seen = cnt;
                    }
                }

                if escrow_save_pending {
                    let st = mem::rd_usize(run.game.base + o::SAVE_STATE_GLOBAL);
                    if st != 0 && mem::rd_u32(st + o::SAVE_REQ_MASK) & 1 == 0 {
                        logf!("escrow save written (mask bit cleared), in mission = {}", run.game.defer_byte(gd) == 1);
                        escrow_save_pending = false;
                    }
                }
                // escrow: set + saved when the deployment talk starts (flag 5003), the last moment the game saves
                let talk = run.game.get_flag(man, o::FLAG_SORTIE_BRIEF) == Some(true);
                // prev_loaded: ignore the talk flag coming back on with a save written during a talk
                if talk && !prev_talk && prev_loaded && run.game.defer_byte(gd) == 0 {
                    run.game.put_flag(man, o::FLAG_ESCROW, true);
                    penalty::arm(gd, "deployment talk");
                    let m = run.game.request_save();
                    logf!("deployment talk: escrow flag SET + save requested (mask before {:?})", m);
                        event::emit(event::Event::Deployment);
                    escrow_save_pending = true;
                }
                prev_talk = talk;

                // garage opened by LOADING a save: flag 1000 (set at every new game) goes 0 -> 1 when the save's
                // flags arrive (the title screen has none). One check per load.
                let loaded = run.game.get_flag(man, o::FLAG_NEW_GAME_DONE) == Some(true);
                if loaded && !prev_loaded {
                    buyback::load(&run.game, man);
                    // dead save: flagged, or an unpaid failure (escrow) whose penalty leaves the player broke.
                    // The second case is the same outcome every time it is loaded, so nothing needs saving:
                    // leave for the title at once, before the garage shows.
                    let broke = if run.game.get_flag(man, o::FLAG_ESCROW) == Some(true) { penalty::arm(gd, "save loaded with escrow") } else { None };
                    if run.game.get_flag(man, o::FLAG_RUN_OVER) == Some(true) || broke.map_or(false, |v| v < 0) {
                        logf!("save loaded: this run is OVER (flag {:?}, projected failure balance {:?}) -> back to the title",
                              run.game.get_flag(man, o::FLAG_RUN_OVER), broke);
                        event::emit(event::Event::GameOver);
                        exit_phase = 1;
                    } else {
                        event::emit(event::Event::SaveLoaded);
                        penalty::CLEANUP_REQUEST.store(true, Relaxed); cap::AFTER_COMMIT.store(false, Relaxed); cap::CHECK.store(true, Relaxed); run.on_garage_open(man, "save loaded");
                    }
                }
                if !loaded && prev_loaded { event::emit(event::Event::Title); }
                prev_loaded = loaded;

                if penalty::RETIRED.swap(0, Relaxed) > 0 { log("death: mission retired automatically (back to the garage)"); }
                // Game over exit, the way the system menu's Return to Title ends a session: the title checks the save
                // FILE for an open session (else it warns that Return to Title / Quit Game was not selected), and the
                // game saves nothing at the title or during missions. So: (1) close the session and keep the game from
                // re-opening it (it does at every map load, then saves), request the session-end save; (2) wait for
                // that save to be written; (3) leave for the title. Timeouts keep a stuck save from blocking the exit.
                match exit_phase {
                    1 => {
                        // Close the session (the game can no longer re-open it: open_session_hook), request the
                        // session-end save and the exit together. The exit sequence fades to black, then waits in its
                        // state 2 until no save is pending or running (exe+12721A0) before unloading: the save is
                        // written behind the black screen, like the system menu's Save and Return to Title.
                        penalty::SESSION_EXIT.store(true, Relaxed);
                        penalty::clear_session_flag(run.game.base);
                        penalty::queue_session_end_save(run.game.base);
                        log("game over: session closed, session-end save + title requested");
                        exit_phase = 3; title_at = tick;
                    }
                    3 => {
                        penalty::clear_session_flag(run.game.base);
                        if !loaded { exit_phase = 0; penalty::SESSION_EXIT.store(false, Relaxed); log("returned to the title screen"); }
                    }
                    _ => {}
                }
                // keep raising the leave-map request (once a second) until the save is unloaded: a request
                // raised during the load may be dropped when the map step starts
                if !loaded { title_at = 0; }
                if title_at != 0 && tick >= title_at {
                    title_at = tick + 10;
                    if penalty::request_title(run.game.base) { log("title request issued (menu leave-map byte set)"); }
                    else { log("title request FAILED (menu manager not available)"); }
                }
                if penalty::GAME_OVER_PENDING.load(Relaxed) {
                    let st = mem::rd_usize(run.game.base + o::SAVE_STATE_GLOBAL);
                    if st != 0 && mem::rd_u32(st + o::SAVE_REQ_MASK) & 1 == 0 {
                        penalty::GAME_OVER_PENDING.store(false, Relaxed);
                        log("game over saved -> back to the title");
                        exit_phase = 1;
                    }
                }
                // DEBUG: %USERPROFILE%/ac6_debug.txt with a command ("reroll" = new shop now, "mission_end" = what a
                // cleared mission does to the shop, "rows_test" = row bitset round trip, "lost" = lost-forever rows,
                // "occur" = [max_occurrences] counts,
                // "reward" = a sortie's reward roll without the grant, "shop <hex ids>" = a shop of these parts, "save" = request a save,
                // "give <hex item id>" = grant an item in the garage, "give2 <hex>" = same even if owned, "force_reward <hex>" = next
                // sortie's reward, "dups" = log the inventory entries of every part owned twice), then deleted
                let dbg_cmd = std::path::Path::new(&std::env::var("USERPROFILE").unwrap_or_default()).join("ac6_debug.txt");
                if tick % 5 == 0 && dbg_cmd.exists() {
                    let cmd = std::fs::read_to_string(&dbg_cmd).unwrap_or_default();
                    let _ = std::fs::remove_file(&dbg_cmd);
                    match cmd.trim() {
                        "reroll" => { log("DEBUG: reroll"); run.reroll(gd, man); prev_state = None; }
                        "mission_end" => { log("DEBUG: mission end"); run.on_mission_end(gd, man); prev_state = None; }
                        "rows_test" => {
                            let g = &run.game;
                            g.write_rows(man, o::ROWS_SOLD, &[0, 5, 317]);
                            g.put_row(man, o::ROWS_SOLD, 100, true);
                            g.put_row(man, o::ROWS_SOLD, 5, false);
                            logf!("DEBUG: rows {:?} has(100)={} has(5)={}", g.read_rows(man, o::ROWS_SOLD), g.has_row(man, o::ROWS_SOLD, 100), g.has_row(man, o::ROWS_SOLD, 5));
                            g.clear_rows(man, o::ROWS_SOLD);
                            logf!("DEBUG: rows after clear {:?}", g.read_rows(man, o::ROWS_SOLD));
                        }
                        "lost" => logf!("DEBUG: lost forever rows {:?}", run.game.read_rows(man, o::ROWS_LOST)),
                        "occur" => {
                            // [max_occurrences]: offers so far of every listed part, and the used-up ones
                            let counts = occur::read(&run.game, man);
                            let lost = run.game.read_rows(man, o::ROWS_LOST);
                            for p in run.pool.iter().filter(|p| p.max > 0) {
                                let gone = if p.is_os() { os::gone(&run.game, man, p.os) } else { lost.contains(&p.idx) };
                                logf!("DEBUG: occur {} ({}): offered {}/{}{}", p.name(), p.idx, occur::count(&counts, p), p.max, if gone { ", never again" } else { "" });
                            }
                            logf!("DEBUG: occur slots in use {}/{}: {:?}", counts.len(), o::OCCUR_SLOTS, counts);
                        }
                        "osnote" => for c in [o::MSG_CAT_MENU_TITLE, o::MSG_CAT_MENU_TEXT] {
                            logf!("DEBUG: unlock notification {}: {:?}", c, msg::shown(c, o::MSG_OS_UNLOCKED));
                        },
                        "os" => {
                            // each OS Tuning: level / max, rollable as a reward, its core expansion item owned
                            let owned = run.game.owned_ids(gd);
                            let cands = run.reward_candidates(gd, man, &[]);
                            for t in os::tunings() {
                                logf!("DEBUG: OS {} {}: level {:?}/{}, in pool {}, rollable {}{}", t.row, os::name(t.row), os::level(gd, t.row), t.max,
                                    run.pool.iter().any(|p| p.os == t.row && p.in_pool()), cands.iter().any(|p| p.os == t.row),
                                    if t.item != 0 { format!(", item owned {}", owned.contains(&t.item)) } else { String::new() });
                            }
                        }
                        "save" => logf!("DEBUG: save requested (mask before {:?})", run.game.request_save()),
                        c if c.starts_with("speffect ") => match c[9..].trim().parse::<i32>() {
                            // `speffect <id>`: a SpEffectParam row applied to the player (game thread, next frame)
                            Ok(id) => { logf!("DEBUG: speffect {} queued for the player", id); event::queue_speffect(id, false); }
                            Err(_) => logf!("DEBUG: bad speffect id {:?}", c),
                        },
                        c if c.starts_with("odds") => {
                            // `odds [N]`: N weighted picks among the reward candidates (pool weights after the chips)
                            let n: usize = c[4..].trim().parse().unwrap_or(10000);
                            let cands = run.reward_candidates(gd, man, &[]);
                            let mut count = vec![0usize; config::get().pools.len()];
                            for _ in 0..n { if let Some(p) = pick_weighted(&mut run.rng, &cands) { count[p.pool as usize] += 1; } }
                            logf!("DEBUG: odds over {} picks: {}", n, config::get().pools.iter().enumerate().filter(|(i, _)| count[*i] > 0)
                                .map(|(i, p)| format!("{} {:.1}% (weight {})", p.name, 100.0 * count[i] as f64 / n as f64, pool_weight(i as u8)))
                                .collect::<Vec<_>>().join(", "));
                        }
                        "reward" => {
                            // a sortie's reward roll, without granting
                            run.reward_parts.clear(); run.reward_sets.clear();
                            run.roll_rewards(gd, man, "DEBUG reward");
                            logf!("DEBUG: rewards {:?}", run.reward_parts.iter().map(|p| format!("{} {}", p.idx, p.name())).collect::<Vec<_>>());
                            run.reward_parts.clear(); run.reward_sets.clear();
                        }
                        c if c.starts_with("shop ") => {
                            // a shop of these parts (hex item ids), baselines from the inventory now, like a reroll
                            let ids: Vec<u32> = c[5..].split_whitespace().filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).collect();
                            let idx: Vec<u16> = ids.iter().filter_map(|id| run.pool.iter().find(|p| p.grant_id() == *id).map(|p| p.idx)).take(config::MAX_SHOP_SLOTS as usize).collect();
                            logf!("DEBUG: shop {:X?} -> rows {:?}", ids, idx);
                            run.game.write_state(man, &idx);
                            dupshop::write(&run.game, man, &run.slot_parts(&idx));
                            run.apply_state(man, &idx, "DEBUG shop"); prev_state = None;
                            run.game.request_save();
                        }
                        c if c.starts_with("watch ") || c == "unwatch" => trace::command(c),
                        // "choice" = the grid of a sortie's reward offers, "parts <hex ids>" = a one-page grid (display only), "vanilla <hex ids>" = the vanilla dialog with them
                        "choice" => {
                            let cfg = config::get();
                            let cands = run.reward_candidates(gd, man, &[]);
                            let sets = choice::roll(&run.game, man, &cands, cfg.parts_per_mission, cfg.reward_choices as usize);
                            let n = sets.len();
                            if n == 0 { log("DEBUG: reward choice: no part left in the pool, no grid"); } else {
                                rewardmenu::set_pages(sets.iter().enumerate().map(|(i, s)| rewardmenu::Page {
                                    pools: s.iter().map(|p| p.pool_label()).collect(),
                                    ..rewardmenu::Page::parts(s.iter().map(|p| p.grant_id()).collect(), format!("CHOISISSEZ UNE PIÈCE ({}/{})", i + 1, n))
                                }).collect());
                                logf!("DEBUG: reward choice grid, {} page(s) of {:?} part(s)", n, sets.iter().map(|s| s.len()).collect::<Vec<_>>());
                                rewardmenu::OPEN_REQUEST.store(true, Relaxed);
                            }
                        }
                        c if c.starts_with("parts ") || c.starts_with("vanilla ") => {
                            rewardmenu::VANILLA.store(c.starts_with("vanilla "), Relaxed);
                            let ids: Vec<u32> = c[c.find(' ').unwrap_or(0) + 1..].split_whitespace().filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).take(12).collect();
                            logf!("DEBUG: parts grid {:X?}", ids);
                            if !ids.is_empty() { rewardmenu::set_pages(vec![rewardmenu::Page::parts(ids, "CHOISISSEZ UNE PIÈCE".to_string())]); rewardmenu::OPEN_REQUEST.store(true, Relaxed); }
                        }
                        c if c.starts_with("give2 ") => match u32::from_str_radix(c[6..].trim().trim_start_matches("0x"), 16) {
                            Ok(id) => { logf!("DEBUG: give2 {:#X} (game thread, even if owned)", id); penalty::GIVE_FORCE.store(true, Relaxed); penalty::GIVE_REQUEST.store(id, Relaxed); }
                            Err(_) => logf!("DEBUG: bad item id {:?}", c),
                        },
                        c if c.starts_with("force_os ") || c.starts_with("force_reward ") => {
                            let os = c.starts_with("force_os ");
                            let ids: Vec<u32> = c[c.find(' ').unwrap_or(0) + 1..].split_whitespace().filter_map(|v| if os { v.parse::<u16>().ok().map(|r| FORCE_OS_TAG | r as u32) }
                                else { u32::from_str_radix(v.trim_start_matches("0x"), 16).ok() }).collect();
                            logf!("DEBUG: next sortie's rewards forced to {:X?}", ids);
                            if let Ok(mut f) = FORCE_REWARD.lock() { *f = ids; }
                        }
                        "dups" => penalty::log_duplicates(gd),
                        "vente" => { log("DEBUG: open Vente"); cap::OPEN_SELL.store(true, Relaxed); }
                        "cap" => { log("DEBUG: inventory cap check"); cap::CHECK.store(true, Relaxed); }
                        c if c.starts_with("give ") => match u32::from_str_radix(c[5..].trim().trim_start_matches("0x"), 16) {
                            Ok(id) => { logf!("DEBUG: give {:#X} (game thread)", id); penalty::GIVE_REQUEST.store(id, Relaxed); }
                            Err(_) => logf!("DEBUG: bad item id {:?}", c),
                        },
                        c => logf!("DEBUG: unknown command {:?}", c),
                    }
                }
                // DEBUG: a file %USERPROFILE%/ac6_dialog_test.txt is shown in the garage dialog, then deleted
                let dbg = std::path::Path::new(&std::env::var("USERPROFILE").unwrap_or_default()).join("ac6_dialog_test.txt");
                if tick % 5 == 0 && dbg.exists() {
                    let txt = std::fs::read_to_string(&dbg).unwrap_or_default();
                    let _ = std::fs::remove_file(&dbg);
                    dialog::show(txt.trim_end());
                    log("DEBUG: test dialog requested");
                }
                let pb = PAUSE_BLOCKED.swap(0, Relaxed);
                if pb > 0 { logf!("pause menu blocked ({} request{})", pb, if pb > 1 { "s" } else { "" }); }

                // the Parts Shop unlocked after a mission end that found it locked: the game sets 3127 ~30 s after
                // 5074, after the commit autosave, so save the new shop state ourselves
                if run.market_wait && run.game.get_flag(man, o::FLAG_MARKET) == Some(true) {
                    log("Parts Shop unlocked -> first shop (save requested)");
                    run.on_mission_end(gd, man); prev_state = None;
                    run.game.request_save();
                }

                // Replay Mission stays locked (menu command gated by this flag)
                if run.game.get_flag(man, o::FLAG_REPLAY_MISSION) == Some(true) {
                    run.game.put_flag(man, o::FLAG_REPLAY_MISSION, false);
                    log("replay mission flag 3134 cleared");
                }

                // (2) mission signal
                let d = run.game.defer_byte(gd);
                let cleared: u8 = match run.game.get_flag(man, o::FLAG_MISSION_CLEARED) { Some(true) => 1, Some(false) => 0, None => { prev_clear = None; 0 } };
                if let Some(p) = prev_defer {
                    if p == 0 && d == 1 {
                        // mission start (sortie): the game is idle here, the give call returns at once.
                        // The dialog content is built at the mission-clear instant, so the reward must
                        // already be queued by then; check_reward re-queues if the game empties the list.
                        event::emit(event::Event::MissionStart);
                        log("defer 0->1 (mission active)"); cap::AFTER_COMMIT.store(false, Relaxed); // a commit without the post-mission sequence (3139) must not block the cap run.market_wait = false; run.reward_handles.clear(); run.reward_parts.clear(); run.reward_sets.clear(); run.mission_ran = true;
                        alive = false; dead = false; cleared_seen = false;
                        msg::GAME_OVER_TEXT.store(false, Relaxed);
                        // escrow should already be in the save (armed during the pre-mission talk); the game
                        // never saves during a mission, so this is only a fallback for the in-memory state
                        if escrow_save_pending { log("WARNING: mission started before the escrow save was written"); }
                        if run.game.get_flag(man, o::FLAG_ESCROW) != Some(true) { log("WARNING: mission started without the escrow flag"); }
                        choice::disarm();
                        run.queue_reward(gd, man, "mission start");
                        trace::reapply("mission start");
                    }
                    if p == 1 && d == 0 {
                        if run.reward_handles.is_empty() && config::get().parts_per_mission > 0 { log("WARNING: mission-end commit with no reward queued"); }
                        log("defer 1->0 (commit)"); run.reward_handles.clear(); choice::disarm();
                        if run.game.get_flag(man, o::FLAG_NEW_GAME_DONE) == Some(true) { event::emit(event::Event::Commit); }
                        // ignore the flip caused by the game shutting down / exiting to title (save no longer loaded)
                        if !cleared_seen && run.game.get_flag(man, o::FLAG_NEW_GAME_DONE) == Some(true) {
                            logf!("mission FAILED ({})", if dead { "death" } else { "quit" });
                            match penalty::failure_balance(gd) {
                                Some(v) if v < 0 => {
                                    logf!("failure balance {} < 0: GAME OVER -> back to the title", v);
                                    event::emit(event::Event::GameOver);
                                    // the session-end save was queued at the death (begin_game_over_exit): wait for it
                                    exit_phase = 1;
                                }
                                _ => run.on_garage_open(man, "returned from a failed mission"),
                            }
                        }
                    }
                    // a dead player's sortie is failed (no retry): the game empties the pending list, nothing to re-queue
                    if p == 1 && d == 1 && !dead { run.check_reward(gd, man); }
                }
                // death: AP reaches 0 during a mission (charged once; re-armed on respawn after Retry)
                if d == 1 && !cleared_seen {
                    if let Some((ap, max, chr)) = run.game.player_ap() {
                        if ap > 0 { if dead { logf!("respawned (AP {}/{})", ap, max); event::emit(event::Event::Respawn); } alive = true; dead = false; alive_chr = chr; }
                        else if alive && !dead && chr == alive_chr {
                            dead = true; alive = false;
                            event::emit(event::Event::Death);
                            match penalty::failure_balance(gd) {
                                Some(v) if v < 0 => {
                                    logf!("DEATH (AP 0): projected balance after the penalty {} < 0 -> GAME OVER", v);
                                    msg::GAME_OVER_TEXT.store(true, Relaxed);
                                }
                                Some(v) => logf!("DEATH (AP 0): projected balance after the penalty {}", v),
                                None => log("DEATH (AP 0)"),
                            }
                        }
                    }
                }
                if prev_clear == Some(0) && cleared == 1 {
                    cleared_seen = true;
                    run.game.put_flag(man, o::FLAG_ESCROW, false);
                    penalty::disarm();
                    log("escrow flag CLEARED (mission cleared)");
                    trace::reapply("mission cleared");
                    logf!("5074 rose (mission cleared); reward queued={}", format!("{:X?}", run.reward_handles));
                    // the results screen shows every offer of this sortie (before the commit autosave)
                    if run.mission_ran { let parts = run.reward_parts.clone(); run.mark_offered(man, &parts, "mission rewards"); }
                    // Reroll NOW: the game autosaves at the commit (~10 s later), so the new shop state
                    // (direct flag writes, no game call) is included in that save.
                    if run.mission_ran { event::emit(event::Event::MissionCleared); }
                    if run.mission_ran {
                        log("mission cleared -> reroll (before the commit autosave)");
                        run.on_mission_end(gd, man); prev_state = None; run.mission_ran = false;
                    } else { log("5074 rose without a mission having run (stale flag on load): ignored"); }
                }

                // the post-mission sequence ended (garage menu back): the inventory cap check of the commit's rewards
                let post = run.game.get_flag(man, o::FLAG_RESULTS) == Some(true);
                if prev_post && !post && cap::AFTER_COMMIT.swap(false, Relaxed) { cap::CHECK.store(true, Relaxed); }
                prev_post = post;
                prev_clear = Some(cleared);
                prev_defer = Some(d);
            }
        }
        if VERBOSE && tick % 40 == 0 { logf!("tick {} pred_calls={} chosen={:?}", tick, PRED_CALLS.load(Relaxed), chosen()); }
        sleep_ms(100);
    }
}

// ---------------------------------------------------------------- win32 glue
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;

extern "system" {
    fn Sleep(ms: u32);
    fn DisableThreadLibraryCalls(module: *mut c_void) -> i32;
    fn GetTickCount64() -> u64;
    fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    fn VirtualProtect(addr: *mut c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    fn FlushInstructionCache(process: *mut c_void, addr: *const c_void, size: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}
fn sleep_ms(ms: u32) { unsafe { Sleep(ms) } }
fn tick_count() -> u64 { unsafe { GetTickCount64() } }

const DLL_PROCESS_ATTACH: u32 = 1;

#[no_mangle]
pub extern "system" fn DllMain(module: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(module);
            // hook memory next to the image now, before the game's heaps take the space within rel32 reach
            if let Some(p) = alloc_near(mem::module_base(), POOL_SIZE) { POOL.store(p, Relaxed); }
        }
        std::thread::spawn(boot);
    }
    1
}
