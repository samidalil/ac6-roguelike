//! In-process memory helpers. The DLL runs inside armoredcore6.exe, so every
//! address is directly dereferenceable; `is_readable` (VirtualQuery) guards pointer chains
//! so a stray unmapped page can never fault the game.

use core::ffi::c_void;

#[repr(C)]
struct MemoryBasicInformation {
    base_address: usize,
    allocation_base: usize,
    allocation_protect: u32,
    partition_id: u16,
    _pad: u16,
    region_size: usize,
    state: u32,
    protect: u32,
    kind: u32,
    _pad2: u32,
}

extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn VirtualQuery(addr: *const c_void, buf: *mut MemoryBasicInformation, len: usize) -> usize;
}

const MEM_COMMIT: u32 = 0x1000;
const PAGE_GUARD: u32 = 0x100;
const PAGE_NOACCESS: u32 = 0x01;
// readable pages
const PAGE_READMASK: u32 = 0x02 /*R*/ | 0x04 /*RW*/ | 0x08 /*WC*/ | 0x20 /*RX*/ | 0x40 /*RWX*/ | 0x80 /*RWXC*/;

/// Runtime module base of armoredcore6.exe (0x140000000 in practice, but resolved live).
pub fn module_base() -> usize {
    unsafe { GetModuleHandleW(core::ptr::null()) as usize }
}

#[inline]
pub unsafe fn rd_u8(a: usize) -> u8 { core::ptr::read_unaligned(a as *const u8) }
#[inline]
pub unsafe fn rd_u16(a: usize) -> u16 { core::ptr::read_unaligned(a as *const u16) }
#[inline]
pub unsafe fn rd_i32(a: usize) -> i32 { core::ptr::read_unaligned(a as *const i32) }
#[inline]
pub unsafe fn rd_u32(a: usize) -> u32 { core::ptr::read_unaligned(a as *const u32) }
#[inline]
pub unsafe fn rd_usize(a: usize) -> usize { core::ptr::read_unaligned(a as *const usize) }
#[inline]
pub unsafe fn wr_i32(a: usize, v: i32) { core::ptr::write_unaligned(a as *mut i32, v) }
#[inline]
pub unsafe fn wr_u32(a: usize, v: u32) { core::ptr::write_unaligned(a as *mut u32, v) }
#[inline]
pub unsafe fn wr_u8(a: usize, v: u8) { core::ptr::write_unaligned(a as *mut u8, v) }

/// True if [addr, addr+len) is entirely in a committed, readable region.
pub fn is_readable(addr: usize, len: usize) -> bool {
    if addr == 0 { return false; }
    let mut mbi: MemoryBasicInformation = unsafe { core::mem::zeroed() };
    let got = unsafe {
        VirtualQuery(
            addr as *const c_void,
            &mut mbi,
            core::mem::size_of::<MemoryBasicInformation>(),
        )
    };
    if got == 0 { return false; }
    if mbi.state != MEM_COMMIT { return false; }
    if (mbi.protect & (PAGE_GUARD | PAGE_NOACCESS)) != 0 { return false; }
    if (mbi.protect & PAGE_READMASK) == 0 { return false; }
    let end = mbi.base_address + mbi.region_size;
    addr + len <= end
}
