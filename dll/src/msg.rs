//! Message lookup override: lets the DLL replace specific UI texts while keeping the game's own font and layout.
//!
//! The game resolves every UI string through MsgRepository::LookupEntry (exe+323FE20):
//!   (repo, version, category, id) -> wide string or null
//! The function is short and self-contained, so we overwrite its entry with an absolute jump to `msg_lookup`,
//! which re-implements it exactly and only intercepts the ids we override.

use super::*;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// "GAME OVER" replaces the mission-failed texts while set.
pub static GAME_OVER_TEXT: AtomicBool = AtomicBool::new(false);

const fn w<const N: usize>(s: &[u8; N]) -> [u16; N] {
    let mut out = [0u16; N];
    let mut i = 0;
    while i < N { out[i] = s[i] as u16; i += 1; }
    out
}
static GAME_OVER_W: [u16; 10] = w(b"GAME OVER\0");

/// (category, id) pairs shown when the player dies: the death-sequence banner and the death menu title.
const MISSION_FAILED_TEXTS: [(u32, u32); 2] = [(200, 254000), (30, 110003004)];

static BASE: AtomicUsize = AtomicUsize::new(0);

/// Text served for `dialog::CUSTOM_MSG_ID` (any category), NUL-terminated UTF-16. Kept alive until replaced.
static CUSTOM: Mutex<Vec<u16>> = Mutex::new(Vec::new());

pub fn set_custom_text(text: &str) {
    let mut w: Vec<u16> = text.encode_utf16().collect();
    w.push(0);
    if let Ok(mut c) = CUSTOM.lock() { *c = w; }
}

/// Lookups of the garage menu's title ("GARAGE"): the garage menu is on screen (event.rs waits for one before its
/// dialog, the post-mission notification and talk being over).
pub static GARAGE_TITLE_SEEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

unsafe extern "system" fn msg_lookup(repo: usize, version: u32, category: u32, id: u32) -> usize {
    if (category, id) == o::MSG_GARAGE_TITLE { GARAGE_TITLE_SEEN.fetch_add(1, Relaxed); }
    if GAME_OVER_TEXT.load(Relaxed) && MISSION_FAILED_TEXTS.contains(&(category, id)) {
        return GAME_OVER_W.as_ptr() as usize;
    }
    if let Some(p) = cap::refused_text(category, id) { return p; }
    if let Some(p) = buyback::confirm_text(category, id) { return p; }
    if id == dialog::CUSTOM_MSG_ID {
        if let Ok(c) = CUSTOM.lock() { if !c.is_empty() { return c.as_ptr() as usize; } }
    }
    if category == o::MSG_CAT_MENU_TEXT && (id == o::MSG_TUTO_ACHAT || id == o::MSG_TUTO_VENTE) {
        return tutorial_text(id).as_ptr() as usize;
    }
    if id == o::MSG_OS_UNLOCKED && (category == o::MSG_CAT_MENU_TITLE || category == o::MSG_CAT_MENU_TEXT) && config::get().os_rewards {
        if let Some(p) = os_unlock_text(repo, version, category) { return p; }
    }
    original(repo, version, category, id)
}

/// exe+323FE20 as the game has it.
unsafe fn original(repo: usize, version: u32, category: u32, id: u32) -> usize {
    if version >= mem::rd_u32(repo + 0x10) || category >= mem::rd_u32(repo + 0x14) { return 0; }
    let per_version = mem::rd_usize(mem::rd_usize(repo + 8) + version as usize * 8);
    if per_version == 0 { return 0; }
    let fmg = mem::rd_usize(per_version + category as usize * 8);
    if fmg == 0 { return 0; }
    let get: unsafe extern "system" fn(usize, u32) -> usize = core::mem::transmute(BASE.load(Relaxed) + o::FN_FMG_GET);
    get(fmg, id)
}

/// The OS Tuning + Arena unlock notification (title, text) without the OS Tuning menu, which stays locked: its menu
/// name (and the "/" joining it to the Arena) is cut from the title, the text lines naming it are dropped. Built once
/// per game text (keyed by its address: the texts are reloaded in the game's language after the title screen); None
/// (vanilla text) when they don't name it.
static OS_UNLOCK: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
unsafe fn os_unlock_text(repo: usize, version: u32, category: u32) -> Option<usize> {
    let src = original(repo, version, category, o::MSG_OS_UNLOCKED);
    if src == 0 { return None; }
    let mut cache = OS_UNLOCK.lock().ok()?;
    if let Some(c) = cache.iter().find(|c| c.0 == src) { return (c.1 != 0).then_some(c.1); }
    let read = |p: usize| -> Option<String> {
        if p == 0 { return None; }
        let p = p as *const u16;
        let mut n = 0;
        while *p.add(n) != 0 && n < 4096 { n += 1; }
        Some(String::from_utf16_lossy(core::slice::from_raw_parts(p, n)))
    };
    let menu = read(original(repo, version, o::MSG_CAT_MENU_TITLE, o::MSG_OS_MENU)).unwrap_or_default();
    let text = read(src).unwrap_or_default();
    let out = if menu.is_empty() || !text.contains(&menu) { None }
        else if category == o::MSG_CAT_MENU_TITLE { Some(text.replace(&format!("{}/", menu), "").replace(&format!("/{}", menu), "")) }
        else { Some(text.split('\n').filter(|l| !l.contains(&menu)).collect::<Vec<_>>().join("\n")) };
    let ptr = out.map_or(0, |t| {
        logf!("OS Tuning locked: unlock notification {} -> {:?}", category, t);
        Box::leak(t.encode_utf16().chain(Some(0)).collect::<Vec<u16>>().into_boxed_slice()).as_ptr() as usize
    });
    cache.push((src, ptr));
    (ptr != 0).then_some(ptr)
}

/// The ACHAT / VENTE first-time tutorials, rewritten for the mod's shop from the settings (sale %, reroll interval,
/// inventory cap). Built once (the settings are read once at boot).
fn tutorial_text(id: u32) -> &'static [u16] {
    static ACHAT: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    static VENTE: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    let c = config::get();
    let renewal = if c.reroll_interval <= 1 { "après chaque mission réussie".to_string() }
        else { format!("toutes les {} missions réussies", c.reroll_interval) };
    let w = |t: String| t.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    if id == o::MSG_TUTO_ACHAT {
        return ACHAT.get_or_init(|| w(format!(
            "Le menu ACHAT propose des pièces d'AC tirées au hasard,\nrenouvelées {}.\n\n\
             • Les pièces que vous avez vendues y sont de nouveau\nen vente jusqu'au prochain renouvellement.\n\
             • Vous pouvez posséder {} pièces au plus, pièces\néquipées comprises : au-delà, l'achat est refusé.",
            renewal, c.max_parts)));
    }
    let pct = if c.part_sell_percent == c.weapon_sell_percent { format!("{} % de sa valeur", c.part_sell_percent) }
        else { format!("{} % de sa valeur\n(arme : {} %)", c.part_sell_percent, c.weapon_sell_percent) };
    VENTE.get_or_init(|| w(format!(
        "Le menu VENTE permet de vendre vos pièces d'AC.\n\n\
         • Une pièce vendue rapporte {}.\n\
         • Elle peut être rachetée à son prix dans le menu ACHAT\njusqu'au prochain renouvellement de la boutique.\n\
         • Au plus {} pièces vendues restent rachetables : au-delà,\nla plus ancienne part à la casse.\n\
         • Plus de {} pièces au garage : vendez-en jusqu'à\nrevenir à la limite.",
        pct, o::SOLD_COPY_SLOTS, c.max_parts)))
}

/// Set the module base before `install` (part names are read at boot, before the hook goes in).
pub fn set_base(base: usize) { BASE.store(base, Relaxed); }

/// A game text (e.g. a part name) as a String: the original lookup, read-only, safe from any thread.
pub unsafe fn text(category: u32, id: u32) -> Option<String> {
    let repo = mem::rd_usize(BASE.load(Relaxed) + o::MSG_REPOSITORY_GLOBAL);
    if repo == 0 { return None; }
    let p = original(repo, 0, category, id) as *const u16;
    if p.is_null() { return None; }
    let mut n = 0;
    while *p.add(n) != 0 && n < 512 { n += 1; }
    Some(String::from_utf16_lossy(core::slice::from_raw_parts(p, n)))
}

/// A text as the game now gets it (our overrides included), for debug logs.
pub unsafe fn shown(category: u32, id: u32) -> Option<String> {
    let repo = mem::rd_usize(BASE.load(Relaxed) + o::MSG_REPOSITORY_GLOBAL);
    if repo == 0 { return None; }
    let p = msg_lookup(repo, 0, category, id) as *const u16;
    if p.is_null() { return None; }
    let mut n = 0;
    while *p.add(n) != 0 && n < 4096 { n += 1; }
    Some(String::from_utf16_lossy(core::slice::from_raw_parts(p, n)))
}

/// Overwrite LookupEntry's first 12 bytes with `mov rax, msg_lookup; jmp rax`.
pub unsafe fn install(base: usize) -> Result<(), String> {
    BASE.store(base, Relaxed);
    let site = base + o::FN_MSG_LOOKUP;
    // expected original: cmp edx,[rcx+10] ; jae ; cmp r8d,[rcx+14] ; jae
    let expect: [u8; 4] = [0x3B, 0x51, 0x10, 0x73];
    let mut cur = [0u8; 4];
    core::ptr::copy_nonoverlapping(site as *const u8, cur.as_mut_ptr(), 4);
    if cur != expect { return Err(format!("unexpected bytes at LookupEntry: {:02X?}", cur)); }
    let mut code = [0u8; 12];
    code[0] = 0x48; code[1] = 0xB8;
    code[2..10].copy_from_slice(&(msg_lookup as *const () as usize as u64).to_le_bytes());
    code[10] = 0xFF; code[11] = 0xE0;
    if !write_code(site, &code) { return Err("VirtualProtect failed".into()); }
    Ok(())
}
