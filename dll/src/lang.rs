//! The mod's own texts in the game's language (RE_NOTES "Localization").
//!
//! Texts are keys in lang/<code>.toml files (dll/src/lang, embedded at build); a lang/ folder next to the DLL can
//! override any key of a language. A missing key falls back to the parent language (es-419 -> es), then English.
//! The language follows the game's: a fixed game text (MSG_LANG_SIGNATURE, "Scanning" in English) is matched
//! against its 13 translations at each use, so the texts follow the game when it reloads its own (they come back in
//! the game's language after the title screen). The settings' [display] language forces one.
//!
//! Placeholders: `{name}` is filled by the caller; `{menu.buy}`, `{menu.sell}`, `{menu.assembly}`, `{menu.os}` and
//! `{ap}` are the game's own texts (its menu names, its AP abbreviation) in the language it shows.

use super::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// (code, the game's msg folder, MSG_LANG_SIGNATURE in that language, embedded file)
pub const LANGS: [(&str, &str, &str, &str); 13] = [
    ("en", "engus", "Scanning", include_str!("lang/en.toml")),
    ("fr", "frafr", "Scan", include_str!("lang/fr.toml")),
    ("ja", "jpnjp", "スキャン", ""),
    ("de", "deude", "Scannen", ""),
    ("it", "itait", "Scansione", ""),
    ("es", "spaes", "Escáner", ""),
    ("es-419", "spaar", "Escaneo", ""),
    ("pt", "porbr", "Varredura", ""),
    ("pl", "polpl", "Skanowanie", ""),
    ("ru", "rusru", "Сканирование", ""),
    ("ko", "korkr", "스캔", ""),
    ("zh-tw", "zhotw", "掃描", ""),
    ("zh-cn", "zhocn", "扫描", ""),
];
const EN: usize = 0;

/// The game texts behind the built-in placeholders: (placeholder, category, id).
const GAME_TERMS: [(&str, u32, u32); 5] = [
    ("{menu.buy}", o::MSG_CAT_MENU_TITLE, o::MSG_TUTO_ACHAT),
    ("{menu.sell}", o::MSG_CAT_MENU_TITLE, o::MSG_TUTO_VENTE),
    ("{menu.assembly}", o::MSG_CAT_MENU_LABEL, o::MSG_MENU_ASSEMBLY),
    ("{menu.os}", o::MSG_CAT_MENU_TITLE, o::MSG_OS_MENU),
    ("{ap}", o::MSG_CAT_MENU_LABEL, o::MSG_AP),
];

/// Per language: its keys (embedded file, then the override file's).
static TABLES: OnceLock<Vec<HashMap<String, String>>> = OnceLock::new();
/// The settings' language (index in LANGS), None = the game's.
static FORCED: OnceLock<Option<usize>> = OnceLock::new();
/// The language last seen in the game's texts (English until the game's texts are up).
static CURRENT: AtomicUsize = AtomicUsize::new(EN);

/// Index in LANGS of a language code or game msg folder name ("fr", "frafr"; any case).
pub fn index(code: &str) -> Option<usize> {
    let c = code.trim().to_ascii_lowercase();
    LANGS.iter().position(|l| l.0 == c || l.1 == c)
}

/// Boot (after config::load): the texts, and the settings' language.
pub fn load() {
    let dir = config::dir().join("lang");
    let tables = LANGS.iter().map(|l| {
        let mut t = parse(l.3, l.0);
        let path = dir.join(format!("{}.toml", l.0));
        if let Ok(text) = std::fs::read_to_string(&path) {
            let o = parse(&text, &path.display().to_string());
            logf!("lang: {} keys from {}", o.len(), path.display());
            t.extend(o);
        }
        t
    }).collect();
    let _ = TABLES.set(tables);
    let want = config::get().language.clone();
    let forced = if want.eq_ignore_ascii_case("auto") || want.is_empty() { None } else {
        let i = index(&want);
        if i.is_none() { logf!("lang: [display] language = {}: unknown, the game's language is used", want); }
        i
    };
    logf!("lang: {}", forced.map_or("the game's language".to_string(), |i| format!("forced to {}", LANGS[i].0)));
    let _ = FORCED.set(forced);
}

/// `key = "text"` lines under `[section]` headers (keys "section.key"); `\n`, `\"` and `\\` escapes; `#` comments.
fn parse(text: &str, file: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut section = String::new();
    for (n, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if line.starts_with('[') {
            if let Some(s) = line[1..].split(']').next() { section = s.trim().to_string(); }
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { logf!("lang {} line {}: ignored", file, n + 1); continue };
        let k = k.trim().trim_matches('"');
        let Some(v) = v.trim().strip_prefix('"') else { logf!("lang {} line {}: a quoted text expected", file, n + 1); continue };
        let mut s = String::new();
        let mut chars = v.chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '"' => { closed = true; break }
                '\\' => match chars.next() { Some('n') => s.push('\n'), Some('t') => s.push('\t'), Some(c) => s.push(c), None => {} },
                c => s.push(c),
            }
        }
        if !closed { logf!("lang {} line {}: unclosed quote", file, n + 1); continue; }
        out.insert(if section.is_empty() { k.to_string() } else { format!("{}.{}", section, k) }, s);
    }
    out
}

/// Index in LANGS of the language the texts are in now: the settings', else the game's.
pub fn current() -> usize {
    if let Some(Some(i)) = FORCED.get() { return *i; }
    let cur = CURRENT.load(Relaxed);
    let Some(t) = (unsafe { msg::text(o::MSG_LANG_SIGNATURE.0, o::MSG_LANG_SIGNATURE.1) }) else { return cur };
    if LANGS[cur].2 == t { return cur; }
    match LANGS.iter().position(|l| l.2 == t) {
        Some(i) => { CURRENT.store(i, Relaxed); logf!("lang: the game's texts are in {} ({})", LANGS[i].1, LANGS[i].0); i }
        None => cur,
    }
}

/// Code of the current language ("fr").
pub fn code() -> &'static str { LANGS[current()].0 }

/// The languages looked in for `lang`: itself, its parent (es-419 -> es), English.
pub fn chain(lang: usize) -> Vec<usize> {
    let mut v = vec![lang];
    if let Some(p) = LANGS[lang].0.split_once('-').and_then(|(p, _)| index(p)) { v.push(p); }
    if !v.contains(&EN) { v.push(EN); }
    v
}

/// The text of `key` in the current language, its placeholders filled from `args` and the game's terms.
pub fn f(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let tables = TABLES.get();
    let raw = chain(current()).into_iter().find_map(|i| tables.and_then(|t| t[i].get(key)).cloned());
    let Some(mut s) = raw else { logf!("lang: no text for {}", key); return key.to_string() };
    for (name, v) in args { s = s.replace(&format!("{{{}}}", name), &v.to_string()); }
    for (tag, cat, id) in GAME_TERMS {
        if s.contains(tag) { s = s.replace(tag, &unsafe { msg::text(cat, id) }.unwrap_or_default()); }
    }
    s
}
/// `f` without arguments.
pub fn t(key: &str) -> String { f(key, &[]) }
/// `f` of `key`, or of `key_one` when `n` is 1 and the language has it.
pub fn n(key: &str, n: impl Into<i64>, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let one = format!("{}_one", key);
    let has_one = TABLES.get().is_some_and(|t| chain(current()).into_iter().any(|i| t[i].contains_key(&one)));
    f(if n.into() == 1 && has_one { &one } else { key }, args)
}
/// Items of a list joined with the language's separator (", ").
pub fn list(items: &[String]) -> String { items.join(&t("list_separator")) }

/// A text served to the game (NUL-terminated UTF-16, kept alive): built once per language and `key` (which names
/// everything the text depends on besides the language).
pub fn wide(key: &str, build: impl FnOnce() -> String) -> usize {
    static CACHE: Mutex<Vec<(usize, String, usize)>> = Mutex::new(Vec::new());
    let lang = current();
    if let Some(p) = CACHE.lock().ok().and_then(|c| c.iter().find(|e| e.0 == lang && e.1 == key).map(|e| e.2)) { return p; }
    let p = Box::leak(build().encode_utf16().chain(Some(0)).collect::<Vec<u16>>().into_boxed_slice()).as_ptr() as usize;
    if let Ok(mut c) = CACHE.lock() { c.push((lang, key.to_string(), p)); }
    p
}
