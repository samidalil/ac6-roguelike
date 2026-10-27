//! The game's "Pièces obtenues" menu (02_GotReward), opened by the DLL: we build the same std::function the game
//! builds for it and hand it to the menu manager. The menu lists the pending obtained-items list (mgr+0x78) and
//! commits it on OK. Game thread only (garage).

use super::*;

/// 0 = nothing; 1 = open as is; any other value = queue that item id (deferred give) first, then open.
pub static OPEN_REQUEST: AtomicU32 = AtomicU32::new(0);
/// Factory std::function vtable rva (0 = exe+392B7D0) and the first captured qword (RE tests).
pub static VTABLE: AtomicU32 = AtomicU32::new(0);
pub static CAPTURE: AtomicU32 = AtomicU32::new(0);

/// MSVC std::function: 0x38 bytes of small-object storage + the pointer to the callable (here the storage itself).
#[repr(C, align(16))]
struct StdFunction { storage: [usize; 7], callable: usize }

pub unsafe fn open_pending(base: usize, game: &Game, gd: usize) {
    // the class reads its movie name after our factory returns: restore the vanilla name a little later
    let left = RESTORE_IN.load(Relaxed);
    if left > 0 { RESTORE_IN.store(left - 1, Relaxed); if left == 1 { write_wide(base + o::GOT_REWARD_MOVIE_NAME, VANILLA_MOVIE); log("reward choice: vanilla movie name restored"); } }
    let req = OPEN_REQUEST.load(Relaxed);
    if req == 0 || mem::rd_usize(base + o::DIALOG_REQUIRED_GLOBAL) == 0 { return; }
    OPEN_REQUEST.store(0, Relaxed);
    if req != 1 {
        let defer = gd + o::GD_ITEM_MGR + o::MGR_DEFER;
        mem::wr_u8(defer, 1);
        let (r, err) = game.give_deferred(gd, req);
        mem::wr_u8(defer, 0); // back to garage mode at once (the DLL's loop reads 0->1 as a sortie)
        logf!("got-reward test: queued {:#X} (rax={} err={}), pending {:X?}", req, r, err, game.pending(gd));
    }
    let mut f = StdFunction { storage: [0; 7], callable: 0 };
    let vt = VTABLE.load(Relaxed) as usize;
    f.storage[0] = if vt == OWN { own_vtable() } else { base + if vt == 0 { o::GOT_REWARD_FACTORY_VTABLE } else { vt } };
    f.storage[1] = CAPTURE.load(Relaxed) as usize;
    logf!("got-reward test: factory vtable {:#X}, capture {}", f.storage[0], f.storage[1]);
    f.callable = f.storage.as_ptr() as usize;
    type FnMake = unsafe extern "system" fn(out: *mut usize, f: *const StdFunction) -> *mut usize;
    type FnPush = unsafe extern "system" fn(job: *mut usize);
    let make: FnMake = core::mem::transmute(base + o::FN_MENU_JOB_FROM_FACTORY);
    let push: FnPush = core::mem::transmute(base + o::FN_MENU_JOB_PUSH);
    let mut job = 0usize;
    make(&mut job, &f);
    logf!("got-reward test: job {:#X}", job);
    if job != 0 { push(&mut job); }
}

// ---- our own factory: the "Pièces obtenues" list built from PARTS (what the results screen does at exe+E9A220)
/// VTABLE value selecting our own factory.
pub const OWN: usize = 0xFFFF_FFFF;
/// Item ids listed by our factory (0 = unused).
pub static PARTS: [AtomicU32; 12] = [const { AtomicU32::new(0) }; 12];

/// The game's vector<u32> with its allocator (exe+2923D10() = the default one).
#[repr(C)]
struct GameVec { alloc: usize, begin: *const u32, end: *const u32, cap: *const u32 }

unsafe extern "system" fn own_copy(this: *const usize, dest: *mut usize) -> *mut usize { *dest = *this; *dest.add(1) = *this.add(1); dest }
unsafe extern "system" fn own_call(_this: usize, out: *mut usize, ctx: usize) -> *mut usize {
    let base = mem::module_base();
    let mut ids: Vec<u32> = PARTS.iter().map(|p| p.load(Relaxed)).filter(|v| *v != 0).collect();
    // grid: the class shows (hidden) and commits nothing but needs one entry; OK on it closes the dialog at once
    if TRUNCATE.load(Relaxed) { ids.truncate(1); }
    GRID_PENDING.store(true, Relaxed);
    let buf: &'static [u32] = Box::leak(ids.into_boxed_slice()); // the menu copies the list; ours just stays alive
    let get_alloc: unsafe extern "system" fn() -> usize = core::mem::transmute(base + o::FN_DEFAULT_ALLOCATOR);
    let v = GameVec { alloc: get_alloc(), begin: buf.as_ptr(), end: buf.as_ptr().add(buf.len()), cap: buf.as_ptr().add(buf.len()) };
    logf!("own factory: parts list {:X?}, ctx {:#X}", buf, ctx);
    let make: unsafe extern "system" fn(*mut usize, usize, *const GameVec) -> *mut usize = core::mem::transmute(base + o::FN_GOT_REWARD_PARTS_MENU);
    // the class names its movie with this string while it is built: ours for this one construction
    let name = base + o::GOT_REWARD_MOVIE_NAME;
    // one hidden single-part job per tile: the game builds each part's page (its content std::function)
    let all: Vec<u32> = PARTS.iter().map(|p| p.load(Relaxed)).filter(|v| *v != 0).collect();
    for (i, id) in all.iter().enumerate() {
        let one: &'static [u32] = Box::leak(vec![*id].into_boxed_slice());
        let v1 = GameVec { alloc: get_alloc(), begin: one.as_ptr(), end: one.as_ptr().add(1), cap: one.as_ptr().add(1) };
        let mut job = 0usize;
        make(&mut job, ctx, &v1);
        let callable = find_content_callable(base, job);
        TILE_CALLABLES[i].store(callable, Relaxed);
        logf!("grid: part {} {:#X}: hidden job {:#X}, content callable {:#X}", i, id, job, callable);
    }
    let swap = OWN_MOVIE.load(Relaxed);
    if swap { write_wide(name, CHOICE_MOVIE); RESTORE_IN.store(60, Relaxed); }
    make(out, ctx, &v)
}
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
/// Open our factory's dialog with our movie instead of 02_GotReward (debug toggle).
pub static OWN_MOVIE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static mut CHOICE_NAME_W: [u16; 13] = [0; 13];
/// Garage frames left before the vanilla movie name is written back (0 = nothing pending).
static RESTORE_IN: AtomicU32 = AtomicU32::new(0);

unsafe fn write_wide(at: usize, s: &str) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut old = 0u32;
    VirtualProtect(at as *mut c_void, w.len() * 2, PAGE_EXECUTE_READWRITE, &mut old);
    core::ptr::copy_nonoverlapping(w.as_ptr(), at as *mut u16, w.len());
    VirtualProtect(at as *mut c_void, w.len() * 2, old, &mut old);
}

/// Boot: make the game preload our movie, under the menu table entry of 02_OnlineArena_GotReward (online only).
pub unsafe fn register_movie(base: usize) {
    let names = core::ptr::addr_of_mut!(CHOICE_NAME_W) as *mut u16;
    for (k, c) in CHOICE_MOVIE.encode_utf16().enumerate() { *names.add(k) = c; }
    let entry = base + o::MENU_TABLE_SPARE_ENTRY;
    let old = mem::rd_usize(entry);
    core::ptr::write_unaligned(entry as *mut usize, names as usize);
    logf!("reward choice movie {} registered in menu table entry exe+{:X} (was {:#X})", CHOICE_MOVIE, o::MENU_TABLE_SPARE_ENTRY, old);
    trace::command(&format!("watch 3 {:X} rw 8 12", names as usize)); // RE: who reads our movie name
}
// ---- the grid: our tiles filled with the game's own part view (RE_NOTES "How the class fills a part tile")
/// Our dialog was built and its tiles are not filled yet.
/// Per tile: the content callable (std::function _Impl, vtable exe+39BF948) of a hidden single-part job.
static TILE_CALLABLES: [AtomicUsize; 12] = [const { AtomicUsize::new(0) }; 12];

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

static GRID_PENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// RE switches: give the class only the first part / fill our tiles.
/// RE: log a real backtrace (unwind data) on the next binds.
pub static BACKTRACES: AtomicU32 = AtomicU32::new(4);
extern "system" { fn RtlCaptureStackBackTrace(skip: u32, n: u32, out: *mut usize, hash: *mut u32) -> u16; }
static TRUNCATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static FILL_TILES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The parts content handle the class just bound its view to (valid during that update).
static LAST_HANDLE: AtomicUsize = AtomicUsize::new(0);
/// Handle of _Content_/SelIdx ("selected/count", read by the movie's script). Leaked on purpose: one per dialog.
static SEL_HANDLE: AtomicUsize = AtomicUsize::new(0);
/// Our dialog's window root component (class + 0x4F0, lives as long as the window).
static ROOT_HANDLE: AtomicUsize = AtomicUsize::new(0);

type FnBind = unsafe extern "system" fn(view: *mut u8, handle: usize) -> usize;
type FnFill = unsafe extern "system" fn(view: *mut u8, record: *const u8) -> usize;
type FnViewDtor = unsafe extern "system" fn(view: *mut u8);
type FnGetChild = unsafe extern "system" fn(parent: usize, out: *mut usize, path: *const u8) -> usize;
type FnSetText = unsafe extern "system" fn(target: usize, text: *const u16);
type FnHandle = unsafe extern "system" fn(handle: *mut usize);

/// A component handle the game writes into (size unknown, generous). Never freed: the game may keep it linked.
unsafe fn new_handle() -> *mut usize { Box::leak(Box::new([0usize; 32])).as_mut_ptr() }

unsafe extern "system" fn bind_hook(view: *mut u8, handle: usize) -> usize {
    let base = mem::module_base();
    let orig: FnBind = core::mem::transmute(base + o::FN_PART_VIEW_BIND);
    let r = orig(view, handle);
    if GRID_PENDING.load(Relaxed) { LAST_HANDLE.store(handle, Relaxed); }
    if BACKTRACES.load(Relaxed) > 0 {
        BACKTRACES.fetch_sub(1, Relaxed);
        let mut frames = [0usize; 24];
        let n = RtlCaptureStackBackTrace(0, 24, frames.as_mut_ptr(), core::ptr::null_mut()) as usize;
        let s: Vec<String> = frames[..n].iter().map(|f| if *f >= base && *f < base + 0x0700_0000 { format!("exe+{:X}", f - base) } else { format!("{:X}", f) }).collect();
        logf!("grid: bind backtrace {}", s.join(" "));
    }
    r
}

unsafe extern "system" fn fill_hook(view: *mut u8, record: *const u8) -> usize {
    let base = mem::module_base();
    let orig: FnFill = core::mem::transmute(base + o::FN_PART_VIEW_FILL);
    let r = orig(view, record);
    if GRID_PENDING.swap(false, Relaxed) { fill_tiles(base, LAST_HANDLE.load(Relaxed), record); }
    r
}

unsafe fn fill_tiles(base: usize, _content: usize, _record: *const u8) {
    let root = ROOT_HANDLE.load(Relaxed);
    if root == 0 { log("grid: no window root"); return; }
    let n = PARTS.iter().filter(|p| p.load(Relaxed) != 0).count();
    let get_child: FnGetChild = core::mem::transmute(base + o::FN_GET_CHILD);
    let prepare: FnHandle = core::mem::transmute(base + o::FN_HANDLE_PREPARE);
    let release: FnHandle = core::mem::transmute(base + o::FN_HANDLE_DTOR);
    for i in 0..n {
        let callable = TILE_CALLABLES[i].load(Relaxed);
        if callable == 0 { logf!("grid: tile {}: no content callable", i); continue; }
        let tile = new_handle();
        let name = format!("_Content_/Tile{:02}\0", i);
        get_child(root, tile, name.as_ptr());
        prepare(tile);
        // the page's content std::function: _Do_call(callable, &component) -> the part view controller (kept)
        let call: unsafe extern "system" fn(usize, *mut usize) -> usize = core::mem::transmute(mem::rd_usize(mem::rd_usize(callable) + 0x10));
        let ctl = call(callable, tile);
        release(tile);
        logf!("grid: tile {} filled (controller {:#X}, handle {:X?})", i, ctl, core::slice::from_raw_parts(tile, 4));
    }
    let sel = new_handle();
    get_child(root, sel, b"_Content_/SelIdx\0".as_ptr());
    SEL_HANDLE.store(sel as usize, Relaxed);
    set_selection(base, 0, n);
}
/// Write "selected/count" into _Content_/SelIdx (game thread).
pub unsafe fn set_selection(base: usize, sel: usize, count: usize) {
    let h = SEL_HANDLE.load(Relaxed);
    if h == 0 { return; }
    let text: Vec<u16> = format!("{}/{}", sel, count).encode_utf16().chain(Some(0)).collect();
    let set_text: FnSetText = core::mem::transmute(base + o::FN_SET_TEXT);
    set_text(h + 8, text.as_ptr());
}

type FnGetChildHook = unsafe extern "system" fn(parent: usize, out: *mut usize, path: *const u8) -> usize;
/// RE: the page function's "_Content_" lookup (rdi = class, rsi = page object at this point).
unsafe extern "system" fn content_lookup_hook(parent: usize, out: *mut usize, path: *const u8) -> usize {
    let page = mem::rd_usize(SAVED_RSI.load(Relaxed));
    let base = mem::module_base();
    let orig: FnGetChildHook = core::mem::transmute(base + o::FN_GET_CHILD);
    let r = orig(parent, out, path);
    if GRID_PENDING.load(Relaxed) { ROOT_HANDLE.store(parent, Relaxed); }
    if PAGE_DUMPS.load(Relaxed) > 0 {
        PAGE_DUMPS.fetch_sub(1, Relaxed);
        let this = parent - 0x4F0;
        logf!("grid: content lookup, class {:#X} page {:#X}; page[0..0x100] {:X?}", this, page, (0..32).map(|k| mem::rd_usize(page + 8 * k)).collect::<Vec<_>>());
        let hits: Vec<String> = (0..0x6A0 / 8).filter_map(|k| { let v = mem::rd_usize(this + 8 * k); if v != 0 && v.abs_diff(page) < 0x800 { Some(format!("+{:X}={:X}", 8 * k, v)) } else { None } }).collect();
        logf!("grid: class fields near the page: {}", hits.join(" "));
        let mut frames = [0usize; 20];
        let n = RtlCaptureStackBackTrace(0, 20, frames.as_mut_ptr(), core::ptr::null_mut()) as usize;
        let s: Vec<String> = frames[..n].iter().map(|f| if *f >= base && *f < base + 0x0700_0000 { format!("exe+{:X}", f - base) } else { format!("{:X}", f) }).collect();
        logf!("grid: page function backtrace {}", s.join(" "));
    }
    r
}
static PAGE_DUMPS: AtomicU32 = AtomicU32::new(3);
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

/// Boot: route the parts view bind / fill calls of the "Pièces obtenues" class through our hooks.
pub unsafe fn install(base: usize) {
    for (site, target, f) in [(o::PART_VIEW_BIND_CALLSITE, o::FN_PART_VIEW_BIND, bind_hook as *const () as usize),
                              (o::PART_VIEW_FILL_CALLSITE, o::FN_PART_VIEW_FILL, fill_hook as *const () as usize)] {
        match install_call_hook(base, site, target, f) {
            Ok(cave) => logf!("reward grid hook installed: exe+{:X} -> cave {:#X}", site, cave),
            Err(e) => logf!("reward grid hook exe+{:X} FAILED: {}", site, e),
        }
    }
    match install_call_hook_saving_rsi(base, o::CONTENT_LOOKUP_CALLSITE, o::FN_GET_CHILD, content_lookup_hook as *const () as usize) {
        Ok(cave) => logf!("reward grid hook installed: exe+{:X} -> cave {:#X} (saves rsi)", o::CONTENT_LOOKUP_CALLSITE, cave),
        Err(e) => logf!("reward grid hook exe+{:X} FAILED: {}", o::CONTENT_LOOKUP_CALLSITE, e),
    }
}