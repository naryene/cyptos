//! RISC-V Physical Memory Protection (PMP)
//!
//! PMP provides per-hart machine-mode control of memory access for lower
//! privilege levels. Reference: RISC-V Privileged Specification v1.12, Chapter 3.7

use core::arch::asm;

/// PMP configuration flags
#[allow(dead_code)]
pub mod flags {
    pub const R: u8 = 1 << 0; // Read
    pub const W: u8 = 1 << 1; // Write
    pub const X: u8 = 1 << 2; // Execute
    pub const A_OFF: u8 = 0 << 3; // Address matching disabled
    pub const A_TOR: u8 = 1 << 3; // Top of Range
    pub const A_NA4: u8 = 2 << 3; // Naturally aligned 4-byte
    pub const A_NAPOT: u8 = 3 << 3; // Naturally aligned power-of-2
    pub const L: u8 = 1 << 7; // Lock (also enforces for M-mode)

    pub const RW: u8 = R | W;
    pub const RX: u8 = R | X;
    pub const RWX: u8 = R | W | X;
}

/// Maximum number of PMP entries (RV64 supports up to 64, QEMU virt has 16)
pub const PMP_COUNT: usize = 16;

/// PMP region descriptor
#[derive(Debug, Clone, Copy)]
pub struct PmpRegion {
    pub base: u64,
    pub size: u64,
    pub flags: u8,
}

impl PmpRegion {
    pub const fn new(base: u64, size: u64, flags: u8) -> Self {
        Self { base, size, flags }
    }

    /// Convert to NAPOT address format.
    /// For NAPOT, pmpaddr = (base + (size/2 - 1)) >> 2
    /// Size must be power of 2 and >= 8
    pub fn to_napot_addr(&self) -> u64 {
        debug_assert!(self.size.is_power_of_two() && self.size >= 8);
        (self.base + (self.size / 2 - 1)) >> 2
    }
}

/// Write to pmpcfg0 (entries 0-7 config)
fn write_pmpcfg0(val: u64) {
    unsafe {
        asm!("csrw pmpcfg0, {}", in(reg) val, options(nostack));
    }
}

/// Write to pmpcfg2 (entries 8-15 config)
fn write_pmpcfg2(val: u64) {
    unsafe {
        asm!("csrw pmpcfg2, {}", in(reg) val, options(nostack));
    }
}

/// Read pmpcfg0
fn read_pmpcfg0() -> u64 {
    let val: u64;
    unsafe {
        asm!("csrr {}, pmpcfg0", out(reg) val, options(nostack));
    }
    val
}

/// Read pmpcfg2
fn read_pmpcfg2() -> u64 {
    let val: u64;
    unsafe {
        asm!("csrr {}, pmpcfg2", out(reg) val, options(nostack));
    }
    val
}

#[allow(unused_macros)]
macro_rules! write_pmpaddr {
    ($idx:literal, $val:expr) => {
        paste::paste! {
            unsafe {
                asm!(concat!("csrw pmpaddr", $idx, ", {}"), in(reg) $val, options(nostack));
            }
        }
    };
}

macro_rules! impl_pmpaddr_write {
    ($($idx:literal),*) => {
        fn write_pmpaddr(idx: usize, val: u64) {
            match idx {
                $(
                    $idx => unsafe {
                        asm!(concat!("csrw pmpaddr", $idx, ", {}"), in(reg) val, options(nostack));
                    },
                )*
                _ => panic!("Invalid PMP index"),
            }
        }
    };
}

impl_pmpaddr_write!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

/// Configure a single PMP entry using NAPOT addressing.
/// Returns true if successful, false if region is invalid.
pub fn configure_region(idx: usize, region: &PmpRegion) -> bool {
    if idx >= PMP_COUNT {
        return false;
    }

    if !region.size.is_power_of_two() || region.size < 8 {
        crate::serial::puts("[pmp] Invalid region size (must be power of 2 >= 8)\n");
        return false;
    }

    if region.base & (region.size - 1) != 0 {
        crate::serial::puts("[pmp] Base not aligned to size\n");
        return false;
    }

    let addr = region.to_napot_addr();
    write_pmpaddr(idx, addr);

    let cfg = region.flags | flags::A_NAPOT;
    set_pmpcfg(idx, cfg);

    true
}

/// Set configuration byte for a specific PMP entry
fn set_pmpcfg(idx: usize, cfg: u8) {
    let shift = (idx % 8) * 8;
    let mask = !(0xFFu64 << shift);
    let val = (cfg as u64) << shift;

    if idx < 8 {
        let current = read_pmpcfg0();
        write_pmpcfg0((current & mask) | val);
    } else {
        let current = read_pmpcfg2();
        write_pmpcfg2((current & mask) | val);
    }
}

/// Clear all PMP entries
pub fn clear_all() {
    write_pmpcfg0(0);
    write_pmpcfg2(0);
    for i in 0..PMP_COUNT {
        write_pmpaddr(i, 0);
    }
}

/// Initialize PMP with default kernel protection.
/// Sets up regions for kernel text (RX), kernel data (RW), and MMIO.
pub fn init() {
    clear_all();

    unsafe extern "C" {
        static _stext: u8;
        static _etext: u8;
        static _sdata: u8;
        static _end: u8;
    }

    let text_start = &raw const _stext as u64;
    let text_end = &raw const _etext as u64;
    let data_start = &raw const _sdata as u64;
    let kernel_end = &raw const _end as u64;

    crate::serial::puts("[pmp] Kernel text: ");
    crate::serial::put_hex(text_start);
    crate::serial::puts(" - ");
    crate::serial::put_hex(text_end);
    crate::serial::puts("\n");

    crate::serial::puts("[pmp] Kernel data: ");
    crate::serial::put_hex(data_start);
    crate::serial::puts(" - ");
    crate::serial::put_hex(kernel_end);
    crate::serial::puts("\n");

    // For now, allow all access to memory (will be restricted when tasks are added)
    // Entry 0: Full memory access (temporary - for kernel development)
    // In production, this would be split into proper regions
    let full_access = PmpRegion::new(
        0,
        1 << 56, // Cover all 56-bit physical address space
        flags::RWX,
    );
    configure_region(0, &full_access);

    crate::serial::puts("[pmp] Initialized with permissive config\n");
}

/// Debug: print current PMP configuration
#[allow(dead_code)]
pub fn dump_config() {
    crate::serial::puts("[pmp] Configuration:\n");
    crate::serial::puts("  pmpcfg0: ");
    crate::serial::put_hex(read_pmpcfg0());
    crate::serial::puts("\n  pmpcfg2: ");
    crate::serial::put_hex(read_pmpcfg2());
    crate::serial::puts("\n");
}
