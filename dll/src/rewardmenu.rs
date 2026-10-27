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