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

unsafe extern "system" fn msg_lookup(repo: usize, version: u32, category: u32, id: u32) -> usize {
    if GAME_OVER_TEXT.load(Relaxed) && MISSION_FAILED_TEXTS.contains(&(category, id)) {
        return GAME_OVER_W.as_ptr() as usize;
    }
    if id == dialog::CUSTOM_MSG_ID {
        if let Ok(c) = CUSTOM.lock() { if !c.is_empty() { return c.as_ptr() as usize; } }
    }
    // original exe+323FE20
    if version >= mem::rd_u32(repo + 0x10) || category >= mem::rd_u32(repo + 0x14) { return 0; }
    let per_version = mem::rd_usize(mem::rd_usize(repo + 8) + version as usize * 8);
    if per_version == 0 { return 0; }
    let fmg = mem::rd_usize(per_version + category as usize * 8);
    if fmg == 0 { return 0; }
    let get: unsafe extern "system" fn(usize, u32) -> usize = core::mem::transmute(BASE.load(Relaxed) + o::FN_FMG_GET);
    get(fmg, id)
}

/// Set the module base before `install` (part names are read at boot, before the hook goes in).
pub fn set_base(base: usize) { BASE.store(base, Relaxed); }

/// A game text (e.g. a part name) as a String: the original lookup, read-only, safe from any thread.
pub unsafe fn text(category: u32, id: u32) -> Option<String> {
    let repo = mem::rd_usize(BASE.load(Relaxed) + o::MSG_REPOSITORY_GLOBAL);
    if repo == 0 { return None; }
    let p = msg_lookup(repo, 0, category, id) as *const u16;
    if p.is_null() { return None; }
    let mut n = 0;
    while *p.add(n) != 0 && n < 512 { n += 1; }
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
