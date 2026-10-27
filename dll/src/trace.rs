//! RE helper: hardware breakpoints (debug registers on every game thread) with a vectored exception handler that
//! logs the registers and the return addresses found on the stack. Debug commands (ac6_debug.txt):
//! `watch <slot 0-3> <addr|exe+rva> <x|w|rw> <len 1/2/4/8> [max hits]`, `unwatch`.

use super::*;
use std::sync::atomic::AtomicBool;

const CTX_SIZE: usize = 0x4D0;
const CONTEXT_DEBUG_REGISTERS: u32 = 0x0010_0010;
const EXCEPTION_SINGLE_STEP: u32 = 0x8000_0004;

#[repr(C, align(16))]
struct Ctx([u8; CTX_SIZE]);
impl Ctx {
    fn u64(&self, o: usize) -> u64 { u64::from_le_bytes(self.0[o..o + 8].try_into().unwrap()) }
    fn set_u64(&mut self, o: usize, v: u64) { self.0[o..o + 8].copy_from_slice(&v.to_le_bytes()); }
}

static SLOT_ADDR: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
static SLOT_LEFT: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
static DR7: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);

extern "system" {
    fn AddVectoredExceptionHandler(first: u32, h: unsafe extern "system" fn(*mut [usize; 2]) -> i32) -> *mut c_void;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Thread32First(snap: *mut c_void, te: *mut ThreadEntry) -> i32;
    fn Thread32Next(snap: *mut c_void, te: *mut ThreadEntry) -> i32;
    fn OpenThread(access: u32, inherit: i32, tid: u32) -> *mut c_void;
    fn SuspendThread(h: *mut c_void) -> u32;
    fn ResumeThread(h: *mut c_void) -> u32;
    fn GetThreadContext(h: *mut c_void, ctx: *mut Ctx) -> i32;
    fn SetThreadContext(h: *mut c_void, ctx: *const Ctx) -> i32;
    fn CloseHandle(h: *mut c_void) -> i32;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
}

#[repr(C)]
struct ThreadEntry { size: u32, usage: u32, tid: u32, owner_pid: u32, base_pri: i32, delta_pri: i32, flags: u32 }

/// Return addresses into the exe found in the first 0x600 bytes of the stack (a call instruction right before).
unsafe fn stack_chain(rsp: usize) -> String {
    let base = mem::module_base();
    let end = base + 0x0700_0000;
    let mut out = String::new();
    let mut n = 0;
    for k in 0..0xC0 {
        let v = mem::rd_usize(rsp + k * 8);
        if v <= base + 0x1000 || v >= end { continue; }
        let call = mem::rd_u8(v - 5) == 0xE8 || mem::rd_u8(v - 2) == 0xFF || mem::rd_u8(v - 3) == 0xFF || mem::rd_u8(v - 6) == 0xFF;
        if !call { continue; }
        let _ = write!(out, " {:X}", v - base);
        n += 1;
        if n >= 18 { break; }
    }
    out
}

unsafe extern "system" fn veh(info: *mut [usize; 2]) -> i32 {
    let rec = (*info)[0];
    let ctx = &mut *((*info)[1] as *mut Ctx);
    if mem::rd_u32(rec) != EXCEPTION_SINGLE_STEP { return 0; }
    let dr6 = ctx.u64(0x68);
    let Some(slot) = (0..4).find(|i| dr6 & (1 << i) != 0 && SLOT_ADDR[*i].load(Relaxed) != 0) else { return 0; };
    let base = mem::module_base() as u64;
    let rip = ctx.u64(0xF8);
    let left = SLOT_LEFT[slot].load(Relaxed);
    if left > 0 {
        SLOT_LEFT[slot].store(left - 1, Relaxed);
        logf!("TRACE slot {} hit: rip exe+{:X} tid {} rax {:X} rcx {:X} rdx {:X} r8 {:X} r9 {:X} rbx {:X} rsi {:X} rdi {:X} rsp {:X} | stack{}",
            slot, rip.wrapping_sub(base), GetCurrentThreadId(), ctx.u64(0x78), ctx.u64(0x80), ctx.u64(0x88), ctx.u64(0xB8), ctx.u64(0xC0),
            ctx.u64(0x90), ctx.u64(0xA8), ctx.u64(0xB0), ctx.u64(0x98), stack_chain(ctx.u64(0x98) as usize));
        let deref = |v: u64| if mem::is_readable(v as usize, 8) { mem::rd_usize(v as usize).wrapping_sub(base as usize) as u64 } else { 0 };
        logf!("TRACE slot {}   [rcx]-base {:X} [rdx]-base {:X} [r8]-base {:X}", slot, deref(ctx.u64(0x80)), deref(ctx.u64(0x88)), deref(ctx.u64(0xB8)));
        let rcx = ctx.u64(0x80) as usize;
        if mem::is_readable(rcx, 0x20) { logf!("TRACE slot {}   rcx[0..4] {:X} {:X} {:X} {:X}", slot, mem::rd_usize(rcx), mem::rd_usize(rcx + 8), mem::rd_usize(rcx + 0x10), mem::rd_usize(rcx + 0x18)); }
        if left == 1 { logf!("TRACE slot {}: hit limit reached (breakpoint stays set but silent)", slot); }
    }
    ctx.set_u64(0x68, 0);
    // execution breakpoint: resume flag so the instruction runs once without re-trapping
    if (DR7.load(Relaxed) >> (16 + slot * 4)) & 3 == 0 {
        let fl = u32::from_le_bytes(ctx.0[0x44..0x48].try_into().unwrap()) | 0x10000;
        ctx.0[0x44..0x48].copy_from_slice(&fl.to_le_bytes());
    }
    -1 // EXCEPTION_CONTINUE_EXECUTION
}

/// Write the current slots into the debug registers of every thread of the process (but this one).
unsafe fn apply() -> u32 {
    if !INSTALLED.swap(true, Relaxed) { AddVectoredExceptionHandler(1, veh); }
    let snap = CreateToolhelp32Snapshot(4, 0); // TH32CS_SNAPTHREAD
    let me = GetCurrentThreadId();
    let pid = GetCurrentProcessId();
    let mut te = ThreadEntry { size: core::mem::size_of::<ThreadEntry>() as u32, usage: 0, tid: 0, owner_pid: 0, base_pri: 0, delta_pri: 0, flags: 0 };
    let mut n = 0;
    let mut ok = Thread32First(snap, &mut te);
    while ok != 0 {
        if te.owner_pid == pid && te.tid != me {
            let h = OpenThread(0x0002 | 0x0008 | 0x0010, 0, te.tid); // suspend/resume, get, set context
            if !h.is_null() {
                SuspendThread(h);
                let mut c = Ctx([0; CTX_SIZE]);
                c.0[0x30..0x34].copy_from_slice(&CONTEXT_DEBUG_REGISTERS.to_le_bytes());
                if GetThreadContext(h, &mut c) != 0 {
                    for i in 0..4 { c.set_u64(0x48 + i * 8, SLOT_ADDR[i].load(Relaxed) as u64); }
                    c.set_u64(0x70, DR7.load(Relaxed) as u64);
                    if SetThreadContext(h, &c) != 0 { n += 1; }
                }
                ResumeThread(h);
                CloseHandle(h);
            }
        }
        ok = Thread32Next(snap, &mut te);
    }
    CloseHandle(snap);
    n
}

/// Re-arm the current slots on every thread (threads created since the last command have none). No-op when unused.
pub unsafe fn reapply(why: &str) {
    if DR7.load(Relaxed) == 0 { return; }
    logf!("TRACE: re-armed on {} threads ({})", apply(), why);
}

fn parse_addr(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some(r) = s.strip_prefix("exe+") { return usize::from_str_radix(r, 16).ok().map(|v| mem::module_base() + v); }
    usize::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// `watch <slot> <addr> <x|w|rw> <len> [max]` / `unwatch`
pub unsafe fn command(cmd: &str) {
    let a: Vec<&str> = cmd.split_whitespace().collect();
    if a.first() == Some(&"unwatch") {
        for s in &SLOT_ADDR { s.store(0, Relaxed); }
        DR7.store(0, Relaxed);
        logf!("TRACE: all breakpoints cleared on {} threads", apply());
        return;
    }
    let (Some(slot), Some(addr)) = (a.get(1).and_then(|v| v.parse::<usize>().ok()).filter(|v| *v < 4), a.get(2).and_then(|v| parse_addr(v))) else {
        logf!("TRACE: bad command {:?}", cmd); return;
    };
    let rw = match a.get(3).copied().unwrap_or("rw") { "x" => 0, "w" => 1, _ => 3 };
    let len = match a.get(4).copied().unwrap_or("8") { "1" => 0, "2" => 1, "4" => 3, _ => 2 };
    let max = a.get(5).and_then(|v| v.parse().ok()).unwrap_or(20);
    let mut d = DR7.load(Relaxed);
    d &= !(3 << (slot * 2)) & !(0xF << (16 + slot * 4));
    d |= 1 << (slot * 2);
    d |= (if rw == 0 { 0 } else { (len << 2) | rw }) << (16 + slot * 4);
    DR7.store(d, Relaxed);
    SLOT_ADDR[slot].store(addr, Relaxed);
    SLOT_LEFT[slot].store(max, Relaxed);
    logf!("TRACE: slot {} = {:#X} (exe+{:X}) {} len {} max {} -> {} threads", slot, addr, addr.wrapping_sub(mem::module_base()),
        ["x", "w", "?", "rw"][rw], a.get(4).copied().unwrap_or("8"), max, apply());
}
