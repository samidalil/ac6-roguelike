//! Shots fired by chips from the player's AC without the weapon equipped (scripts: `drone(x, y, z)`,
//! `shoot(weapon, behavior, x, y, z[, tx, ty, tz])`; RE_NOTES "Drone on kill" and "Hidden weapon slots").
//! - exe+6F9430 (producer = chr + 0x1F8, BehaviorParam_PC id, -1, *location, 2, weapon slot, *a7, f32, f32, 1, weapon
//!   slot, *a12, *a13, *a14, index, *a16) is what the weapon fire and the animation events call; the producer sets the
//!   team. One call = one shot: the weapon's own firing logic (ammo, bursts, charge, spin-up) never runs. The location in
//!   its explicit mode (dummy poly -1, mode 1, a 3x4 matrix with the translation in w) puts the shot anywhere; the
//!   matrix's third column is the direction it flies (homing shots, drones and missiles, ignore it and go for the
//!   player's lock target). A shot's attack row may push the shooter back (AtkParam_Pc shooterKnockbackMPS /
//!   shooterFlyKnockbackMPS: the plasma cannon ~10 m): chips fire their own copies without it (plasma_echo.toml).
//!   The other structs come from a template of one real drone call, with the owner handle
//!   (chr +8), the weapon slot and the slot's entry in module 38 (+0x140 + 0x40 x slot) patched in.
//! - The weapon slot must hold the weapon: the internal slots SLOTS (unused by the game, no HUD cell) are loaded with it
//!   the way the game sets its internal slots (exe+903E69..903EB9: descriptor exe+15D6BE0 / exe+15D3370(desc, 0,
//!   equipId), exe+906590(slot - 0x10, &desc, 0), then exe+905AA0(module)), ~15 us. A slot keeps its weapon until another
//!   weapon needs it (the least recently used one goes); shots already fired keep flying after a swap. The module is
//!   rebuilt at each load, so the slot is checked before every shot.

use super::*;
use std::sync::Mutex;
use std::time::Instant;

const FN_BEHAVIOR: usize = 0x6F9430;
const FN_DESC_INIT: usize = 0x15D6BE0;
const FN_DESC_WEAPON: usize = 0x15D3370;
const FN_SET_SLOT: usize = 0x906590;
const FN_WEAPONS_REFRESH: usize = 0x905AA0;
const DRONE_WEAPON: u32 = 30140100;     // LASER DRONE Vvc-700LD (Right)
const DRONE_BEHAVIOR: i32 = 131410000;  // its BehaviorParam_PC row
/// Internal weapon slots for chip shots: 19..31 are unset by the game and shown nowhere (16 / 17 show in the HUD's arm
/// cells; 4-7 / 10-12 hold placeholders the game may use).
const SLOTS: core::ops::RangeInclusive<usize> = 19..=31;
const BUF: usize = 0x100;
type FnBehavior = unsafe extern "system" fn(usize, i32, i32, usize, i32, i32, usize, f32, f32, u32, u32, usize, usize, usize, i32, usize) -> u64;

const T_A7: [u8; 0x80] = [0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0x92, 0x42, 0x00, 0x00, 0x00, 0x00, 0x10, 0xb8, 0x6a, 0xf6, 0xf4, 0x7f, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x13, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x13, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const T_A12: [u8; 0x80] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0x92, 0x42, 0x00, 0x00, 0x00, 0x00, 0x10, 0xb8, 0x6a, 0xf6, 0xf4, 0x7f, 0x00, 0x00, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x13, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x13, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const T_A13: [u8; 0x80] = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x13, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x7c, 0xf0, 0xb0, 0xbb, 0xce, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const T_A16: [u8; 0x80] = [0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x87, 0x92, 0x42, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x25, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

/// A shot: weapon (equipId), its BehaviorParam_PC row, world position, the point it flies to (None: world +z; homing
/// shots ignore it), index (arg 15: the drone number for drones).
#[derive(Clone, Copy)]
pub struct Shot { pub weapon: u32, pub behavior: i32, pub pos: [f32; 3], pub at: Option<[f32; 3]>, pub index: i32 }
static QUEUE: Mutex<Vec<Shot>> = Mutex::new(Vec::new());
/// A shot on the game thread, next frame (dropped without a player AC).
pub fn shoot(s: Shot) { QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(s); }
/// A laser drone at this world position (it goes for the player's lock target).
pub fn drone(p: [f32; 3]) {
    let n = QUEUE.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|s| s.weapon == DRONE_WEAPON).count() as i32;
    shoot(Shot { weapon: DRONE_WEAPON, behavior: DRONE_BEHAVIOR, pos: p, at: None, index: n % 6 + 1 });
}

/// Last use of each slot of SLOTS (game thread).
static LAST_USE: Mutex<[Option<Instant>; 13]> = Mutex::new([None; 13]);

#[repr(C, align(16))] struct B([u8; BUF]);

/// The slot of SLOTS for `weapon` in the player's weapon module `m`: one that holds it, else an empty one, else the
/// least recently used one, loaded with it.
unsafe fn slot_for(base: usize, m: usize, weapon: u32) -> Option<usize> {
    let id = |k: usize| mem::rd_u32(m + o::WEAPON_SLOT0 + k * o::WEAPON_SLOT_STRIDE + o::SLOT_ID);
    let mut used = LAST_USE.lock().unwrap_or_else(|e| e.into_inner());
    let k = SLOTS.clone().find(|&k| id(k) == weapon)
        .or_else(|| SLOTS.clone().find(|&k| id(k) == u32::MAX))
        .or_else(|| SLOTS.clone().min_by_key(|&k| used[k - SLOTS.start()]))?;
    if id(k) != weapon && !load_slot(base, m, k, weapon) { return None; }
    used[k - SLOTS.start()] = Some(Instant::now());
    Some(k)
}

/// `weapon` in internal slot `k` of the weapon module `m`, set like the game's internal slots.
unsafe fn load_slot(base: usize, m: usize, k: usize, weapon: u32) -> bool {
    let slot = m + o::WEAPON_SLOT0 + k * o::WEAPON_SLOT_STRIDE;
    let mut d = B([0; BUF]);
    let p = d.0.as_mut_ptr() as usize;
    mem::wr_i32(p + 4, -1); mem::wr_i32(p + 0x10, -1); mem::wr_i32(p + 0x18, -1);
    let init: unsafe extern "system" fn(usize, i32) -> usize = core::mem::transmute(base + FN_DESC_INIT);
    init(p + 0x18, -1);
    mem::wr_i32(p + 0x28, -1); mem::wr_i32(p + 0x2C, 0);
    let desc: unsafe extern "system" fn(usize, i32, u32) -> usize = core::mem::transmute(base + FN_DESC_WEAPON);
    desc(p, 0, weapon);
    let set: unsafe extern "system" fn(usize, usize, u32) -> u64 = core::mem::transmute(base + FN_SET_SLOT);
    set(slot - 0x10, p, 0);
    let refresh: unsafe extern "system" fn(usize) -> u64 = core::mem::transmute(base + FN_WEAPONS_REFRESH);
    refresh(m);
    let ok = mem::rd_u32(slot + o::SLOT_ID) == weapon;
    logf!("shot: weapon slot {} {} {}", k, if ok { "loaded with" } else { "NOT loaded with" }, weapon);
    ok
}

/// The map's characters: WorldChrMan +0xA760 / +0xA768 (begin / end of 0x18-byte entries, ChrIns at +0: MTs, drones,
/// helicopters...) and +0xAB58 / +0xAB68 (begin / end of ChrIns pointers: the player and the opposing ACs).
pub unsafe fn characters(base: usize) -> Vec<usize> {
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    if wcm == 0 { return Vec::new(); }
    let mut v = Vec::new();
    for (begin, end_at, stride) in [(o::WCM_CHRS, 8, 0x18), (o::WCM_ACS, 0x10, 8)] {
        let (mut e, end) = (mem::rd_usize(wcm + begin), mem::rd_usize(wcm + begin + end_at));
        if e == 0 || end < e || end - e > 0x10_0000 { continue; }
        while e < end { let c = mem::rd_usize(e); if c != 0 && !v.contains(&c) { v.push(c); } e += stride; }
    }
    v
}
/// The player's lock target: its ChrIns (found by its handle among `characters`).
pub unsafe fn lock_target(base: usize) -> Option<usize> {
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    let chr = if wcm == 0 { 0 } else { mem::rd_usize(wcm + o::WCM_PLAYER) };
    if chr == 0 { return None; }
    let h = mem::rd_usize(chr + o::CHR_LOCK_TARGET);
    if h == usize::MAX { return None; }
    characters(base).into_iter().find(|&c| mem::rd_usize(c + o::CHR_HANDLE) == h)
}

/// Game thread, every frame (penalty.rs frame hook): the queued shots.
pub unsafe fn tick(base: usize) {
    let q: Vec<Shot> = core::mem::take(&mut *QUEUE.lock().unwrap_or_else(|e| e.into_inner()));
    if q.is_empty() { return; }
    let wcm = mem::rd_usize(base + o::WORLDCHRMAN_GLOBAL);
    let chr = if wcm == 0 { 0 } else { mem::rd_usize(wcm + o::WCM_PLAYER) };
    if chr == 0 { return; }
    let mods = mem::rd_usize(chr + o::PLAYER_MODULES);
    let (m, m38) = (mem::rd_usize(mods + 8 * o::CHR_MOD_WEAPONS), mem::rd_usize(mods + 8 * 38));
    if m == 0 || m38 == 0 || mem::rd_usize(m) != base + o::WEAPONS_VTABLE { return; }
    let handle = mem::rd_u32(chr + 8);
    let behavior: FnBehavior = core::mem::transmute(base + FN_BEHAVIOR);
    let put32 = |b: &mut B, off: usize, v: u32| b.0[off..off + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |b: &mut B, off: usize, v: u64| b.0[off..off + 8].copy_from_slice(&v.to_le_bytes());
    for s in q {
        let Some(slot) = slot_for(base, m, s.weapon) else { continue };
        let (k, p) = (slot as u32, s.pos);
        let entry = (m38 + 0x140 + 0x40 * slot) as u64;
        let (mut a4, mut a7, mut a12, mut a13, mut a16) = (B([0; BUF]), B([0; BUF]), B([0; BUF]), B([0; BUF]), B([0; BUF]));
        a7.0[..0x80].copy_from_slice(&T_A7); a12.0[..0x80].copy_from_slice(&T_A12);
        a13.0[..0x80].copy_from_slice(&T_A13); a16.0[..0x80].copy_from_slice(&T_A16);
        put64(&mut a7, 0x10, entry); put32(&mut a7, 0x28, handle); put32(&mut a7, 0x30, k); put32(&mut a7, 0x48, handle); put32(&mut a7, 0x50, k);
        put64(&mut a12, 0x30, entry); put32(&mut a12, 0x48, handle); put32(&mut a12, 0x50, k); put32(&mut a12, 0x68, handle); put32(&mut a12, 0x70, k);
        put32(&mut a13, 0x08, handle); put32(&mut a13, 0x10, k);
        put32(&mut a16, 0x08, k);
        put32(&mut a4, 0, u32::MAX); a4.0[4] = 1;
        // rotation: the third column along the direction to `at` (the first from the world up), else identity
        let mut r = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        if let Some(t) = s.at {
            let norm = |v: [f32; 3]| { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4); [v[0] / n, v[1] / n, v[2] / n] };
            let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let f = norm([t[0] - p[0], t[1] - p[1], t[2] - p[2]]);
            let x = norm(cross([0.0, 1.0, 0.0], f));
            let y = cross(f, x);
            for i in 0..3 { r[i] = [x[i], y[i], f[i]]; }
        }
        let f: [f32; 12] = [r[0][0], r[0][1], r[0][2], p[0], r[1][0], r[1][1], r[1][2], p[1], r[2][0], r[2][1], r[2][2], p[2]];
        for (i, v) in f.iter().enumerate() { a4.0[0x10 + 4 * i..0x14 + 4 * i].copy_from_slice(&v.to_le_bytes()); }
        let res = behavior(chr + 0x1F8, s.behavior, -1, a4.0.as_mut_ptr() as usize, 2, k as i32, a7.0.as_mut_ptr() as usize, 0.0, 0.0, 1, k,
            a12.0.as_mut_ptr() as usize, a13.0.as_mut_ptr() as usize, 0, s.index, a16.0.as_mut_ptr() as usize);
        logf!("shot: {} (behavior {}, slot {}) at ({:.1}, {:.1}, {:.1}){} -> {}", s.weapon, s.behavior, k, p[0], p[1], p[2],
            s.at.map_or(String::new(), |t| format!(" to ({:.1}, {:.1}, {:.1})", t[0], t[1], t[2])), res & 0xFF);
    }
}
