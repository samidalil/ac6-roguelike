//! Standalone OK dialog with our own text, in the game's own dialog box (RE_NOTES "Standalone dialog").
//!
//! The talk scripts' command 17 (5 arguments, FromSoft's OpenGenericDialog) builds a menu job from the script's
//! argument list: exe+DC1650(&job, args) reads arguments 1..5 synchronously, the menu framework supplies the owner;
//! exe+DC0BC0(&handle) opens the job and exe+DBA200(&job) resets the temporary (no reference counting). We pass a
//! fake argument list (our own vtable) whose message id is answered by the message lookup hook (msg.rs).
//! Arguments 1 1 0 1 give an information dialog with a single OK button. Game thread only.

use super::*;
use std::sync::atomic::AtomicBool;

/// One script argument as the game reads it: value (int bits) and a kind tag at +8 (2 = int, 1 = float).
#[repr(C)]
struct EsdArg { value: u32, _pad: u32, kind: u32, _pad2: u32 }
const fn int(v: u32) -> EsdArg { EsdArg { value: v, _pad: 0, kind: 2, _pad2: 0 } }

/// Message id given to the dialog; msg.rs answers it (category 30) with the text set by `show`.
pub const CUSTOM_MSG_ID: u32 = 990_000_001;
/// [0] = command id 17, [1..=5] = the five arguments (kind, message id, buttons, options, mode).
static ARGS: [EsdArg; 6] = [int(17), int(1), int(CUSTOM_MSG_ID), int(1), int(0), int(1)];
static ZERO: EsdArg = int(0);

// The argument-list interface: slot 2 = command id, slot 3 = argument count (+1), slot 4 = get(i). The other
// slots are never called by the dialog builder; they return a harmless zero argument just in case.
unsafe extern "system" fn a_unused(_this: usize, _a: usize) -> *const EsdArg { &ZERO }
unsafe extern "system" fn a_cmd(_this: usize) -> u32 { 17 }
unsafe extern "system" fn a_count(_this: usize) -> u32 { 6 }
unsafe extern "system" fn a_get(_this: usize, i: u32) -> *const EsdArg { &ARGS[(i as usize).min(5)] }

#[repr(C)]
struct FakeArgs { vtable: *const usize }
static mut VTABLE: [usize; 16] = [0; 16];
static mut FAKE: FakeArgs = FakeArgs { vtable: core::ptr::null() };

static OPEN_REQUEST: AtomicBool = AtomicBool::new(false);

/// Text of the requested dialog: a second `show` before it opens adds its text under the first (one dialog).
static TEXT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Show `text` in an OK dialog on the next garage frame.
pub fn show(text: &str) {
    let mut t = TEXT.lock().unwrap_or_else(|e| e.into_inner());
    if OPEN_REQUEST.load(Relaxed) && !t.is_empty() { t.push_str("\n\n"); } else { t.clear(); }
    t.push_str(text);
    msg::set_custom_text(&t);
    OPEN_REQUEST.store(true, Relaxed);
}

/// Game thread (garage): open the requested dialog; returns its job (for exe+DC4AE0, "still open?"). Waits (request kept) until the builder's required global exists:
/// a penalty applied right after a save load can request the dialog before the garage is fully up.
pub unsafe fn open_pending(base: usize) -> Option<(usize, usize)> {
    if !OPEN_REQUEST.load(Relaxed) || mem::rd_usize(base + o::DIALOG_REQUIRED_GLOBAL) == 0 { return None; }
    OPEN_REQUEST.store(false, Relaxed);
    let vt = core::ptr::addr_of_mut!(VTABLE) as *mut usize;
    for i in 0..16 { *vt.add(i) = a_unused as *const () as usize; }
    *vt.add(2) = a_cmd as *const () as usize;
    *vt.add(3) = a_count as *const () as usize;
    *vt.add(4) = a_get as *const () as usize;
    FAKE.vtable = vt;
    type FnBuild = unsafe extern "system" fn(out: *mut [usize; 3], args: *const FakeArgs) -> *mut [usize; 3];
    type FnOpen = unsafe extern "system" fn(h: *mut [usize; 3]) -> u8;
    type FnReset = unsafe extern "system" fn(h: *mut [usize; 3]);
    let build: FnBuild = core::mem::transmute(base + o::FN_MENU_JOB_GENERIC_DIALOG);
    let open: FnOpen = core::mem::transmute(base + o::FN_MENU_JOB_OPEN);
    let reset: FnReset = core::mem::transmute(base + o::FN_MENU_JOB_RESET);
    let mut job = [0usize; 3];
    build(&mut job, core::ptr::addr_of!(FAKE));
    let mut h = [base + o::MENU_JOB_HANDLE_VTABLE, job[1], job[2]];
    let ok = open(&mut h);
    let opened = (job[1], job[2]);
    reset(&mut job);
    if ok == 0 { log("dialog: the menu job did not open"); return None; }
    Some(opened)
}
