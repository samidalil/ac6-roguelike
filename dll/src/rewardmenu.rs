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
pub static PARTS: [AtomicU32; 8] = [const { AtomicU32::new(0) }; 8];

/// The game's vector<u32> with its allocator (exe+2923D10() = the default one).
#[repr(C)]
struct GameVec { alloc: usize, begin: *const u32, end: *const u32, cap: *const u32 }

unsafe extern "system" fn own_copy(this: *const usize, dest: *mut usize) -> *mut usize { *dest = *this; *dest.add(1) = *this.add(1); dest }
unsafe extern "system" fn own_call(_this: usize, out: *mut usize, ctx: usize) -> *mut usize {
    let base = mem::module_base();
    let ids: Vec<u32> = PARTS.iter().map(|p| p.load(Relaxed)).filter(|v| *v != 0).collect();
    let buf: &'static [u32] = Box::leak(ids.into_boxed_slice()); // the menu copies the list; ours just stays alive
    let get_alloc: unsafe extern "system" fn() -> usize = core::mem::transmute(base + o::FN_DEFAULT_ALLOCATOR);
    let v = GameVec { alloc: get_alloc(), begin: buf.as_ptr(), end: buf.as_ptr().add(buf.len()), cap: buf.as_ptr().add(buf.len()) };
    logf!("own factory: parts list {:X?}, ctx {:#X}", buf, ctx);
    let make: unsafe extern "system" fn(*mut usize, usize, *const GameVec) -> *mut usize = core::mem::transmute(base + o::FN_GOT_REWARD_PARTS_MENU);
    // the class names its movie with this string while it is built: ours for this one construction
    let name = base + o::GOT_REWARD_MOVIE_NAME;
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