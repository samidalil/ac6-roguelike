//! The reward choice grid: the game's "Pièces obtenues" class (02_GotReward, parts mode) shown with our own movie
//! (menu/02_rwdchoice.gfx), one page per reward, each page a grid of up to 12 parts filled by the game's own part view.
//! At the results screen the game's call that opens "Pièces obtenues" opens our grid instead (choice.rs gives the
//! pages); just before the game commits the pending list, choice.rs takes the parts not picked out of it.
//! RE_NOTES "Native reward choice".

use super::*;

/// One page of the grid: the parts it shows (the highlighted one is that page's pick) and its title.
#[derive(Clone)]
pub struct Page { pub ids: Vec<u32>, pub title: String }

/// Pages of the next grid we build.
static PAGES: std::sync::Mutex<Vec<Page>> = std::sync::Mutex::new(Vec::new());
/// Per page, per part: the content callable (std::function _Impl, vtable exe+39BF948) of a hidden single-part job.
static CALLABLES: std::sync::Mutex<Vec<Vec<usize>>> = std::sync::Mutex::new(Vec::new());
/// Highlighted part of each page (the pick), 0 until moved.
static PICKS: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];
/// Highlighted part of page `page` (0 when the page was never shown).
pub fn picked(page: usize) -> usize { PICKS.get(page).map_or(0, |p| p.load(Relaxed) as usize) }

/// Garage debug: open a grid of these pages (display only, nothing granted).
pub static OPEN_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_pages(pages: Vec<Page>) { if let Ok(mut p) = PAGES.lock() { *p = pages; } }
/// Garage debug: the next open shows the vanilla "Pièces obtenues" instead of the grid.
pub static VANILLA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// MSVC std::function: 0x38 bytes of small-object storage + the pointer to the callable (here the storage itself).
#[repr(C, align(16))]
struct StdFunction { storage: [usize; 7], callable: usize }

/// Game thread, every frame: the delayed movie name restore, and the debug open.
pub unsafe fn open_pending(base: usize) {
    let left = RESTORE_IN.load(Relaxed);
    if left > 0 { RESTORE_IN.store(left - 1, Relaxed); if left == 1 { restore_movie_name(base); } }
    if !OPEN_REQUEST.load(Relaxed) || mem::rd_usize(base + o::DIALOG_REQUIRED_GLOBAL) == 0 { return; }
    OPEN_REQUEST.store(false, Relaxed);
    let mut f = StdFunction { storage: [0; 7], callable: 0 };
    f.storage[0] = own_vtable();
    f.callable = f.storage.as_ptr() as usize;
    type FnMake = unsafe extern "system" fn(out: *mut usize, f: *const StdFunction) -> *mut usize;
    type FnPush = unsafe extern "system" fn(job: *mut usize);
    let make: FnMake = core::mem::transmute(base + o::FN_MENU_JOB_FROM_FACTORY);
    let push: FnPush = core::mem::transmute(base + o::FN_MENU_JOB_PUSH);
    let mut job = 0usize;
    make(&mut job, &f);
    if job != 0 { push(&mut job); }
}

/// The game's vector<u32> with its allocator (exe+2923D10() = the default one).
#[repr(C)]
struct GameVec { alloc: usize, begin: *const u32, end: *const u32, cap: *const u32 }
type FnPartsMenu = unsafe extern "system" fn(out: *mut usize, ctx: usize, list: *const GameVec) -> *mut usize;

/// Build our grid's job (out) from PAGES: the class gets one part per page (its pages and OK button), each page's
/// content is then replaced by a grid (fill_hook). Every part also gets a hidden single-part job (never pushed) whose
/// page content callable fills that part's tile.
unsafe fn build(base: usize, out: *mut usize, ctx: usize) -> *mut usize {
    let make: FnPartsMenu = core::mem::transmute(base + o::FN_GOT_REWARD_PARTS_MENU);
    let get_alloc: unsafe extern "system" fn() -> usize = core::mem::transmute(base + o::FN_DEFAULT_ALLOCATOR);
    let pages = PAGES.lock().map(|p| p.clone()).unwrap_or_default();
    if VANILLA.swap(false, Relaxed) {
        // debug: the vanilla "Pièces obtenues" with these parts (one per page), for comparison
        let ids: &'static [u32] = Box::leak(pages.iter().flat_map(|p| p.ids.clone()).collect::<Vec<_>>().into_boxed_slice());
        let v = GameVec { alloc: get_alloc(), begin: ids.as_ptr(), end: ids.as_ptr().add(ids.len()), cap: ids.as_ptr().add(ids.len()) };
        return make(out, ctx, &v);
    }
    let mut callables = Vec::new();
    for (p, page) in pages.iter().enumerate() {
        let mut row = Vec::new();
        for id in &page.ids {
            let one: &'static [u32] = Box::leak(vec![*id].into_boxed_slice());
            let v1 = GameVec { alloc: get_alloc(), begin: one.as_ptr(), end: one.as_ptr().add(1), cap: one.as_ptr().add(1) };
            let mut job = 0usize;
            make(&mut job, ctx, &v1);
            let c = find_content_callable(base, job);
            if c == 0 { logf!("grid: page {} part {:#X}: no content callable", p, id); }
            row.push(c);
        }
        callables.push(row);
    }
    if let Ok(mut c) = CALLABLES.lock() { *c = callables; }
    for p in &PICKS { p.store(0, Relaxed); }
    if let Ok(mut s) = SEEN_PAGES.lock() { s.clear(); }
    let firsts: &'static [u32] = Box::leak(pages.iter().map(|p| p.ids[0]).collect::<Vec<_>>().into_boxed_slice());
    let v = GameVec { alloc: get_alloc(), begin: firsts.as_ptr(), end: firsts.as_ptr().add(firsts.len()), cap: firsts.as_ptr().add(firsts.len()) };
    logf!("grid: {} page(s) {:X?}", pages.len(), pages.iter().map(|p| p.ids.clone()).collect::<Vec<_>>());
    // the class names its window with this string while it is built: ours until the last page is up
    write_wide(base + o::GOT_REWARD_MOVIE_NAME, CHOICE_MOVIE);
    RESTORE_IN.store(0, Relaxed);
    ACTIVE.store(true, Relaxed);
    make(out, ctx, &v)
}

// ---- our std::function (garage debug open)
unsafe extern "system" fn own_copy(this: *const usize, dest: *mut usize) -> *mut usize { *dest = *this; *dest.add(1) = *this.add(1); dest }
unsafe extern "system" fn own_call(_this: usize, out: *mut usize, ctx: usize) -> *mut usize { build(mem::module_base(), out, ctx) }
unsafe extern "system" fn own_type(_this: usize) -> usize { 0 }
unsafe extern "system" fn own_delete(_this: usize, _dealloc: u32) {}
unsafe extern "system" fn own_get(this: usize) -> usize { this + 8 }

static mut OWN_VTABLE: [usize; 6] = [0; 6];
/// MSVC _Func_base: _Copy, _Move, _Do_call, _Target_type, _Delete_this, _Get.
unsafe fn own_vtable() -> usize {
    let vt = core::ptr::addr_of_mut!(OWN_VTABLE) as *mut usize;
    *vt = own_copy as *const () as usize;
    *vt.add(1) = own_copy as *const () as usize;
    *vt.add(2) = own_call as *const () as usize;
    *vt.add(3) = own_type as *const () as usize;
    *vt.add(4) = own_delete as *const () as usize;
    *vt.add(5) = own_get as *const () as usize;
    vt as usize
}

// ---- our own movie (menu/02_rwdchoice.gfx in the mod package)
/// Movie names (same length: the class's string is patched in place).
const VANILLA_MOVIE: &str = "02_GotReward";
const CHOICE_MOVIE: &str = "02_RwdChoice";
static mut CHOICE_NAME_W: [u16; 13] = [0; 13];
/// Frames left before the vanilla movie name is written back (0 = nothing pending).
static RESTORE_IN: AtomicU32 = AtomicU32::new(0);

unsafe fn write_wide(at: usize, s: &str) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut old = 0u32;
    VirtualProtect(at as *mut c_void, w.len() * 2, PAGE_EXECUTE_READWRITE, &mut old);
    core::ptr::copy_nonoverlapping(w.as_ptr(), at as *mut u16, w.len());
    VirtualProtect(at as *mut c_void, w.len() * 2, old, &mut old);
}
unsafe fn restore_movie_name(base: usize) {
    if mem::rd_u16(base + o::GOT_REWARD_MOVIE_NAME + 6) != 'R' as u16 { return; } // "02_R" = ours
    write_wide(base + o::GOT_REWARD_MOVIE_NAME, VANILLA_MOVIE);
    log("reward choice: vanilla movie name restored");
}

/// Boot: make the game preload our movie, under the menu table entry of 02_OnlineArena_GotReward (online only).
pub unsafe fn register_movie(base: usize) {
    let names = core::ptr::addr_of_mut!(CHOICE_NAME_W) as *mut u16;
    for (k, c) in CHOICE_MOVIE.encode_utf16().enumerate() { *names.add(k) = c; }
    let entry = base + o::MENU_TABLE_SPARE_ENTRY;
    let old = mem::rd_usize(entry);
    core::ptr::write_unaligned(entry as *mut usize, names as usize);
    logf!("reward choice movie {} registered in menu table entry exe+{:X} (was {:#X})", CHOICE_MOVIE, o::MENU_TABLE_SPARE_ENTRY, old);
}

// ---- the grid: our tiles filled with the game's own part view (RE_NOTES "How the class fills a part tile")
/// Breadth-first search (3 levels, 0x400 bytes per object) from a job for a pointer P whose [P] is the content
/// callable vtable: that P is the page's std::function _Impl.
unsafe fn find_content_callable(base: usize, job: usize) -> usize {
    let want = base + o::PAGE_CONTENT_CALLABLE_VTABLE;
    let mut level = vec![job];
    let mut seen = std::collections::HashSet::new();
    for _ in 0..4 {
        let mut next = Vec::new();
        for obj in level {
            if obj < 0x10000 || !seen.insert(obj) || !mem::is_readable(obj, 0x400) { continue; }
            if mem::rd_usize(obj) == want { return obj; }
            for k in 0..0x80 { let v = mem::rd_usize(obj + 8 * k); if v > 0x10000 && v & 7 == 0 { next.push(v); } }
        }
        level = next;
        if level.len() > 4000 { level.truncate(4000); }
    }
    0
}

/// Our grid is up and some of its pages were not shown yet.
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A page of our grid just bound its content (its window root): fill it at the class's next fill.
static NEW_PAGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Page objects seen so far, in order: a page's index is its rank here.
static SEEN_PAGES: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());
/// Page being shown.
static PAGE: AtomicUsize = AtomicUsize::new(0);
/// Handle of _Content_/SelIdx ("selected/count", read by the movie's script). Leaked on purpose: one per page.
static SEL_HANDLE: AtomicUsize = AtomicUsize::new(0);
/// The page's window root component (class + 0x4F0).
static ROOT_HANDLE: AtomicUsize = AtomicUsize::new(0);

type FnFill = unsafe extern "system" fn(view: *mut u8, record: *const u8) -> usize;
type FnGetChild = unsafe extern "system" fn(parent: usize, out: *mut usize, path: *const u8) -> usize;
type FnSetText = unsafe extern "system" fn(target: usize, text: *const u16);
type FnHandle = unsafe extern "system" fn(handle: *mut usize);

/// A component handle the game writes into (size unknown, generous). Never freed: the game may keep it linked.
unsafe fn new_handle() -> *mut usize { Box::leak(Box::new([0usize; 32])).as_mut_ptr() }

unsafe extern "system" fn fill_hook(view: *mut u8, record: *const u8) -> usize {
    let base = mem::module_base();
    let orig: FnFill = core::mem::transmute(base + o::FN_PART_VIEW_FILL);
    let r = orig(view, record);
    if NEW_PAGE.swap(false, Relaxed) { fill_page(base, PAGE.load(Relaxed)); }
    r
}

unsafe fn fill_page(base: usize, page: usize) {
    let root = ROOT_HANDLE.load(Relaxed);
    let Some(info) = PAGES.lock().ok().and_then(|p| p.get(page).cloned()) else { return };
    let callables = CALLABLES.lock().ok().and_then(|c| c.get(page).cloned()).unwrap_or_default();
    let get_child: FnGetChild = core::mem::transmute(base + o::FN_GET_CHILD);
    let prepare: FnHandle = core::mem::transmute(base + o::FN_HANDLE_PREPARE);
    let release: FnHandle = core::mem::transmute(base + o::FN_HANDLE_DTOR);
    let set_text: FnSetText = core::mem::transmute(base + o::FN_SET_TEXT);
    let n = info.ids.len().min(12);
    for i in 0..n {
        let callable = callables.get(i).copied().unwrap_or(0);
        if callable == 0 { continue; }
        let tile = new_handle();
        let name = format!("_Content_/Tile{:02}\0", i);
        get_child(root, tile, name.as_ptr());
        prepare(tile);
        // the page's content std::function: _Do_call(callable, &component) fills the part view
        let call: unsafe extern "system" fn(usize, *mut usize) -> usize = core::mem::transmute(mem::rd_usize(mem::rd_usize(callable) + 0x10));
        call(callable, tile);
        release(tile);
    }
    let title = new_handle();
    get_child(root, title, b"Title\0".as_ptr());
    let text: Vec<u16> = info.title.encode_utf16().chain(Some(0)).collect();
    set_text(title as usize + 8, text.as_ptr());
    release(title);
    let sel = new_handle();
    get_child(root, sel, b"_Content_/SelIdx\0".as_ptr());
    SEL_HANDLE.store(sel as usize, Relaxed);
    SEL.store(0, Relaxed);
    COUNT.store(n as u32, Relaxed);
    for a in &ACTIONS { a.store(0, Relaxed); } // ignore what was pressed before the page opened
    GRID_OPEN.store(true, Relaxed);
    set_selection(base, 0, n);
    let pages = PAGES.lock().map(|p| p.len()).unwrap_or(0);
    logf!("grid: page {}/{} filled ({} part(s))", page + 1, pages, n);
    if page + 1 >= pages { ACTIVE.store(false, Relaxed); RESTORE_IN.store(60, Relaxed); }
}
/// Write "selected/count" into _Content_/SelIdx (game thread).
unsafe fn set_selection(base: usize, sel: usize, count: usize) {
    let h = SEL_HANDLE.load(Relaxed);
    if h == 0 { return; }
    let text: Vec<u16> = format!("{}/{}", sel, count).encode_utf16().chain(Some(0)).collect();
    let set_text: FnSetText = core::mem::transmute(base + o::FN_SET_TEXT);
    set_text(h + 8, text.as_ptr());
}

/// The page function's "_Content_" lookup (rsi = the page object at this point): a page of our grid opens.
unsafe extern "system" fn content_lookup_hook(parent: usize, out: *mut usize, path: *const u8) -> usize {
    let page = mem::rd_usize(SAVED_RSI.load(Relaxed));
    let orig: FnGetChild = core::mem::transmute(mem::module_base() + o::FN_GET_CHILD);
    let r = orig(parent, out, path);
    if ACTIVE.load(Relaxed) {
        if let Ok(mut seen) = SEEN_PAGES.lock() {
            let idx = seen.iter().position(|p| *p == page).unwrap_or_else(|| { seen.push(page); seen.len() - 1 });
            PAGE.store(idx, Relaxed);
        }
        ROOT_HANDLE.store(parent, Relaxed);
        NEW_PAGE.store(true, Relaxed);
    }
    r
}
/// Address of the qword where the content lookup cave stores the caller's rsi (the page object).
static SAVED_RSI: AtomicUsize = AtomicUsize::new(0);

/// Like install_call_hook, but the cave first saves rsi (callee-saved, the caller's page object) to a slot.
unsafe fn install_call_hook_saving_rsi(base: usize, site_rva: usize, target_rva: usize, f: usize) -> Result<usize, String> {
    let site = base + site_rva;
    let expect_rel = (target_rva as i64 - (site_rva as i64 + 5)) as i32;
    if mem::rd_u8(site) != 0xE8 || mem::rd_i32(site + 1) != expect_rel { return Err(format!("unexpected bytes at exe+{:X}", site_rva)); }
    let cave = alloc_near_page(base).ok_or("no near page for the cave")?;
    let slot = cave + 0x80;
    let mut code: Vec<u8> = vec![0x48, 0x89, 0x35]; // mov [rip+d32], rsi
    code.extend_from_slice(&((slot as i64 - (cave as i64 + 7)) as i32).to_le_bytes());
    code.extend_from_slice(&[0x48, 0xB8]); code.extend_from_slice(&(f as u64).to_le_bytes()); // mov rax, f
    code.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
    core::ptr::copy_nonoverlapping(code.as_ptr(), cave as *mut u8, code.len());
    SAVED_RSI.store(slot, Relaxed);
    let mut patch = [0u8; 5];
    patch[0] = 0xE8;
    patch[1..5].copy_from_slice(&((cave as i64 - (site as i64 + 5)) as i32).to_le_bytes());
    if !write_code(site, &patch) { return Err("VirtualProtect failed".into()); }
    Ok(cave)
}

/// Results screen: the game opens "Pièces obtenues" with its pending parts list; with a reward choice armed, our
/// grid opens instead.
unsafe extern "system" fn results_parts_hook(out: *mut usize, ctx: usize, list: *const GameVec) -> *mut usize {
    let base = mem::module_base();
    let l = &*list;
    let n = (l.end as usize).saturating_sub(l.begin as usize) / 4;
    let ids: Vec<u32> = if n > 0 && n < 256 { core::slice::from_raw_parts(l.begin, n).to_vec() } else { Vec::new() };
    if let Some(pages) = choice::pages(&ids) {
        logf!("results: parts list {:X?} -> reward choice grid", ids);
        set_pages(pages);
        return build(base, out, ctx);
    }
    let orig: FnPartsMenu = core::mem::transmute(base + o::FN_GOT_REWARD_PARTS_MENU);
    orig(out, ctx, list)
}

type FnCommit = unsafe extern "system" fn(mgr: usize, commit: u32, obj: usize) -> usize;
/// The game commits the pending obtained items (OK of the results' parts dialog): take the parts not picked out first,
/// then unlock the core expansions left in the list.
unsafe extern "system" fn commit_hook(mgr: usize, commit: u32, obj: usize) -> usize {
    if commit & 0xFF != 0 { choice::prune(mgr); expansion::commit(mgr); }
    GRID_OPEN.store(false, Relaxed);
    let base = mem::module_base();
    restore_movie_name(base);
    let orig: FnCommit = core::mem::transmute(base + o::FN_PENDING_COMMIT);
    orig(mgr, commit, obj)
}

/// Boot: the hooks of the grid (page fill, content lookup), of the results' parts dialog and of the commit.
pub unsafe fn install(base: usize) {
    for (site, target, f) in [(o::PART_VIEW_FILL_CALLSITE, o::FN_PART_VIEW_FILL, fill_hook as *const () as usize),
                              (o::RESULTS_PARTS_MENU_CALLSITE, o::FN_GOT_REWARD_PARTS_MENU, results_parts_hook as *const () as usize),
                              (o::PENDING_COMMIT_CALLSITE, o::FN_PENDING_COMMIT, commit_hook as *const () as usize)] {
        match install_call_hook(base, site, target, f) {
            Ok(cave) => logf!("reward grid hook installed: exe+{:X} -> cave {:#X}", site, cave),
            Err(e) => logf!("reward grid hook exe+{:X} FAILED: {}", site, e),
        }
    }
    match install_call_hook_saving_rsi(base, o::CONTENT_LOOKUP_CALLSITE, o::FN_GET_CHILD, content_lookup_hook as *const () as usize) {
        Ok(cave) => logf!("reward grid hook installed: exe+{:X} -> cave {:#X} (saves rsi)", o::CONTENT_LOOKUP_CALLSITE, cave),
        Err(e) => logf!("reward grid hook exe+{:X} FAILED: {}", o::CONTENT_LOOKUP_CALLSITE, e),
    }
    install_input(base);
}

// ---- selection: the movie gets no input, so the DLL moves the highlight from the game's own menu actions (pad,
// keyboard: the game maps both); the highlighted part is the page's pick (the class's own OK goes to the next page /
// closes)
static GRID_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SEL: AtomicU32 = AtomicU32::new(0);
static COUNT: AtomicU32 = AtomicU32::new(0);
const COLS: u32 = 3;
const IN_UP: u32 = 1; const IN_DOWN: u32 = 2; const IN_LEFT: u32 = 4; const IN_RIGHT: u32 = 8; const IN_OK: u32 = 16;

// The game's menu actions (RE_NOTES "Menu input"): each frame the input mapper calls exe+29F23F0(repeater, input,
// ..., dt) which turns the held virtual inputs of the bitset [input+0x7C8] into press + auto-repeat pulses (0.2 s, then
// every 1/15 s). Hooked at its entry: after it, the bits are ORed into ACTIONS until the next tick takes them.
const ACT_OK: u32 = 0x155; const ACT_RIGHT: u32 = 0x156; const ACT_LEFT: u32 = 0x159; const ACT_DOWN: u32 = 0x15C; const ACT_UP: u32 = 0x15F;
static ACTIONS: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];
static REPEAT_TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
type FnRepeat = unsafe extern "system" fn(usize, usize, usize, usize, f32) -> usize;
unsafe extern "system" fn repeat_hook(a: usize, input: usize, c: usize, d: usize, dt: f32) -> usize {
    let orig: FnRepeat = core::mem::transmute(REPEAT_TRAMPOLINE.load(Relaxed));
    let r = orig(a, input, c, d, dt);
    let bits = mem::rd_usize(input + o::INPUT_ACTION_BITS);
    if bits > 0x10000 { for (k, slot) in ACTIONS.iter().enumerate() { let w = mem::rd_u32(bits + 4 * k); if w != 0 { slot.fetch_or(w, Relaxed); } } }
    r
}
/// Hook a function entry: its first `n` bytes (`expect`, no rip-relative code) move to a trampoline that jumps back.
pub unsafe fn install_entry_hook(base: usize, rva: usize, expect: &[u8], f: usize) -> Result<usize, String> {
    let at = base + rva;
    if (0..expect.len()).any(|i| mem::rd_u8(at + i) != expect[i]) { return Err(format!("unexpected bytes at exe+{:X}", rva)); }
    let page = alloc_near_page(base).ok_or("no near page for the cave")?;
    let mut stub = vec![0x48, 0xB8]; stub.extend_from_slice(&(f as u64).to_le_bytes()); stub.extend_from_slice(&[0xFF, 0xE0]); // mov rax, f; jmp rax
    core::ptr::copy_nonoverlapping(stub.as_ptr(), page as *mut u8, stub.len());
    let tramp = page + 0x40;
    let mut t = expect.to_vec();
    let back = tramp + t.len();
    t.push(0xE9); t.extend_from_slice(&(((at + expect.len()) as i64 - (back as i64 + 5)) as i32).to_le_bytes());
    core::ptr::copy_nonoverlapping(t.as_ptr(), tramp as *mut u8, t.len());
    let mut patch = vec![0x90u8; expect.len()];
    patch[0] = 0xE9; patch[1..5].copy_from_slice(&((page as i64 - (at as i64 + 5)) as i32).to_le_bytes());
    if !write_code(at, &patch) { return Err("VirtualProtect failed".into()); }
    Ok(tramp)
}
pub unsafe fn install_input(base: usize) {
    // mov [rsp+10],rbp; mov [rsp+18],rsi; push rdi; sub rsp,30
    let prologue = [0x48, 0x89, 0x6C, 0x24, 0x10, 0x48, 0x89, 0x74, 0x24, 0x18, 0x57, 0x48, 0x83, 0xEC, 0x30];
    match install_entry_hook(base, o::FN_INPUT_REPEAT, &prologue, repeat_hook as *const () as usize) {
        Ok(t) => { REPEAT_TRAMPOLINE.store(t, Relaxed); logf!("menu input hook installed: exe+{:X}", o::FN_INPUT_REPEAT); }
        Err(e) => logf!("menu input hook exe+{:X} FAILED: {}", o::FN_INPUT_REPEAT, e),
    }
}
/// The game's menu actions since the last tick (pulses: press + auto-repeat): directions and confirm.
unsafe fn read_input() -> u32 {
    let acts: Vec<u32> = ACTIONS.iter().map(|a| a.swap(0, Relaxed)).collect();
    let has = |id: u32| acts[(id >> 5) as usize] >> (id & 31) & 1 != 0;
    let mut pulse = 0;
    if has(ACT_UP) { pulse |= IN_UP; }
    if has(ACT_DOWN) { pulse |= IN_DOWN; }
    if has(ACT_RIGHT) { pulse |= IN_RIGHT; }
    if has(ACT_LEFT) { pulse |= IN_LEFT; }
    if has(ACT_OK) { pulse |= IN_OK; }
    pulse
}

/// Game thread, every frame: move the highlight of the page shown (that part becomes the page's pick). Confirm goes to
/// the class's OK (next page / close): the page's handles die with it, so nothing more is written until the next fill.
pub unsafe fn tick(base: usize) {
    if !GRID_OPEN.load(Relaxed) { for a in &ACTIONS { a.store(0, Relaxed); } return; }
    let pulse = read_input();
    if pulse & IN_OK != 0 { GRID_OPEN.store(false, Relaxed); return; }
    if pulse == 0 { return; }
    let (n, mut sel) = (COUNT.load(Relaxed), SEL.load(Relaxed));
    if pulse & IN_LEFT != 0 && sel % COLS > 0 { sel -= 1; }
    if pulse & IN_RIGHT != 0 && sel % COLS < COLS - 1 && sel + 1 < n { sel += 1; }
    if pulse & IN_UP != 0 && sel >= COLS { sel -= COLS; }
    if pulse & IN_DOWN != 0 && sel + COLS < n { sel += COLS; }
    if sel != SEL.load(Relaxed) {
        SEL.store(sel, Relaxed);
        if let Some(p) = PICKS.get(PAGE.load(Relaxed)) { p.store(sel, Relaxed); }
        set_selection(base, sel as usize, n as usize);
    }
}