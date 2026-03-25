//! RISC-V Physical Memory Protection (PMP)
//!
//! PMP provides per-hart machine-mode control of memory access for lower
//! privilege levels. Reference: RISC-V Privileged Specification v1.12, Chapter 3.7

use core::arch::asm;

/// PMP configuration flags
#[allow(dead_code)]
pub mod flags {
    pub const R: u8 = 0b0000_0001 << 0; // Read
    pub const W: u8 = 0b0000_0001 << 1; // Write
    pub const X: u8 = 0b0000_0001 << 2; // Execute
    pub const A_OFF: u8 = 0b0000_0000 << 3; // Address matching disabled
    pub const A_TOR: u8 = 0b0000_0001 << 3; // Top of Range
    pub const A_NA4: u8 = 0b0000_0010 << 3; // Naturally aligned 4-byte
    pub const A_NAPOT: u8 = 0b0000_0011 << 3; // Naturally aligned power-of-2
    pub const L: u8 = 0b0000_0001 << 7; // Lock (also enforces for M-mode)

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
    pub fn napot_addr(self) -> u64 {
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

macro_rules! impl_pmpaddr_read {
    ($($idx:literal),*) => {
        /// Read a pmpaddr register by index (0-15).
        pub fn read_pmpaddr(idx: usize) -> u64 {
            match idx {
                $(
                    $idx => {
                        let val: u64;
                        unsafe {
                            asm!(concat!("csrr {}, pmpaddr", $idx), out(reg) val, options(nostack));
                        }
                        val
                    },
                )*
                _ => panic!("Invalid PMP index"),
            }
        }
    };
}

impl_pmpaddr_read!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

/// Write a PMP configuration for task-switchable entries (4-15) only.
///
/// Entries 0-3 are kernel-locked (L bit set) and MUST NOT be overwritten on
/// context switch — a locked entry cannot be modified until reset, so any
/// write attempt is silently ignored by hardware. We skip them entirely to
/// preserve the kernel protection invariant.
///
/// For pmpcfg0 (covers entries 0-7): read current value, keep lower 32 bits
/// (entries 0-3, locked), replace upper 32 bits (entries 4-7) with new task cfg.
/// For pmpcfg2 (covers entries 8-15): replace entirely with new task cfg.
pub fn load_task_config(regions: &[PmpRegion]) {
    // Step 1: Disable task entries 4-7 by zeroing upper 32 bits of pmpcfg0,
    // while preserving locked entries 0-3 in the lower 32 bits.
    let current_cfg0 = read_pmpcfg0();
    // Mask out entries 4-7 (upper 32 bits), keep entries 0-3 (lower 32 bits).
    let kernel_cfg0_bits = current_cfg0 & 0x0000_0000_FFFF_FFFF;
    write_pmpcfg0(kernel_cfg0_bits);
    // Disable entries 8-15 entirely.
    write_pmpcfg2(0);

    // Step 2: Write pmpaddr registers for entries 4-15.
    let mut task_cfg0_high: u64 = 0; // bits [63:32] of pmpcfg0 → entries 4-7
    let mut cfg2: u64 = 0; // pmpcfg2 → entries 8-15

    // Iterate only entries 4-15 from the provided regions slice.
    for idx in 4..PMP_COUNT {
        // regions is indexed 0-based; entry 4 maps to regions[4] if present.
        let region = if idx < regions.len() {
            regions[idx]
        } else {
            PmpRegion::new(0, 0, 0)
        };

        if region.size == 0 {
            write_pmpaddr(idx, 0);
            continue;
        }

        let addr = region.napot_addr();
        write_pmpaddr(idx, addr);

        let cfg_byte = (region.flags | flags::A_NAPOT) as u64;
        // Entry N occupies byte (N%8)*8 within the 64-bit cfg register.
        let shift = (idx % 8) * 8;
        if idx < 8 {
            // Entries 4-7: placed in bits [63:32] of pmpcfg0.
            task_cfg0_high |= cfg_byte << shift;
        } else {
            // Entries 8-15: placed in pmpcfg2.
            cfg2 |= cfg_byte << shift;
        }
    }

    // Step 3: Write updated configs — preserve locked entries 0-3 in pmpcfg0.
    write_pmpcfg0(kernel_cfg0_bits | task_cfg0_high);
    write_pmpcfg2(cfg2);
}

/// Read current PMP configuration into a fixed-size array (for debugging/save).
/// Only reconstructs the region descriptor, not full encoding detail.
#[allow(dead_code)]
pub fn save_current_config(out: &mut [PmpRegion; PMP_COUNT]) {
    let cfg0 = read_pmpcfg0();
    let cfg2 = read_pmpcfg2();

    for (idx, entry) in out.iter_mut().enumerate() {
        let cfg_val = if idx < 8 {
            ((cfg0 >> ((idx % 8) * 8)) & 0xFF) as u8
        } else {
            ((cfg2 >> ((idx % 8) * 8)) & 0xFF) as u8
        };
        let addr = read_pmpaddr(idx);
        // Store raw values — base/size reconstruction is complex, store encoded form
        *entry = PmpRegion {
            base: addr << 2, // approximation: pmpaddr << 2 ≈ base for NAPOT
            size: 8,         // placeholder — actual decode not needed for our use case
            flags: cfg_val,
        };
    }
}

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

    let addr = region.napot_addr();
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

/// Returns the 4 locked kernel PMP regions installed during `init()`.
///
/// These entries cover kernel-critical MMIO regions with the L bit set,
/// preventing modification until reset and enforcing access permissions for
/// M-mode. Kernel RAM (0x8000_0000) is intentionally NOT covered: M-mode
/// gets default-allow (no PMP match = full access), while U-mode gets
/// default-deny — ensuring U-mode cannot access kernel memory without an
/// explicit per-task PMP entry.
pub fn kernel_entry_regions() -> [PmpRegion; 4] {
    // Obtain kernel memory boundaries from linker symbols.
    unsafe extern "C" {
        static _stext: u8;
        static _user_text_end: u8;
        static _eheap: u8;
    }
    // SAFETY: linker symbols are valid read-only pointers; we only take their address.
    let stext = &raw const _stext as u64;
    let user_text_end = &raw const _user_text_end as u64;
    let eheap = &raw const _eheap as u64;

    // Entry 0: A_OFF anchor — pmpaddr[0] = _stext>>2.
    // Provides the lower bound for the TOR entry 1. No access protection itself.
    // Entry 1: TOR X+L — [_stext, _user_text_end). Kernel + user text, execute-only locked.
    //   - X only (no R): U-mode `load` from kernel text triggers Load Access Fault.
    //   - Without R: kernel text is protected from U-mode reads while still executable.
    //   - M-mode is also restricted to execute (no data reads from .text), which is
    //     acceptable because kernel data lives in .rodata/.data (covered by entry 2).
    // Entry 2: TOR RW+L — [_user_text_end, _eheap). Kernel data/bss/stack/heap, locked.
    // Entry 3: NAPOT RW+L — UART NS16550A MMIO (0x1000_0000, 4 KB), locked.
    //
    // CLINT (0x0200_0000) is below _stext so M-mode gets default-allow (no PMP match).
    // The PmpRegion struct stores TOR entries as (top_address, 0, flags) — init() detects
    // A_TOR in flags and programs pmpaddr[i] = top_address>>2, pmpcfg[i] = flags.
    [
        // Entry 0: anchor (A_OFF), pmpaddr = _stext>>2.
        PmpRegion::new(stext, 0, flags::A_OFF),
        // Entry 1: kernel text + user task code TOR [_stext, _user_text_end), X+L.
        // Execute-only: U-mode cannot read kernel text; load from 0x8000_0000 faults.
        PmpRegion::new(user_text_end, 0, flags::X | flags::L | flags::A_TOR),
        // Entry 2: kernel data TOR [_user_text_end, _eheap), RW+L.
        PmpRegion::new(eheap, 0, flags::RW | flags::L | flags::A_TOR),
        // Entry 3: UART MMIO NAPOT (0x1000_0000, 4 KB, RW, locked).
        PmpRegion::new(0x1000_0000, 0x0000_1000, flags::RW | flags::L),
    ]
}

/// Initialize PMP with locked kernel memory protection entries (0-3).
///
/// Configures entries 0-3 with the L (lock) bit, enforcing access control for
/// both M-mode and U-mode and preventing modification until reset. Task-specific
/// entries (4-15) are left disabled here; `load_task_config` populates them on
/// each context switch.
pub fn init() {
    clear_all();

    let regions = kernel_entry_regions();
    for (i, region) in regions.iter().enumerate() {
        let addr_mode = region.flags & 0b0001_1000; // bits 4:3
        if addr_mode == flags::A_TOR {
            // TOR entry: pmpaddr[i] = top_address >> 2, pmpcfg[i] = flags (already encodes A_TOR).
            write_pmpaddr(i, region.base >> 2);
            set_pmpcfg(i, region.flags);
        } else if region.size > 0 {
            // Standard NAPOT region — use configure_region.
            configure_region(i, region);
        } else if addr_mode == flags::A_OFF && region.base > 0 {
            // A_OFF anchor: write pmpaddr[i] = region.base >> 2, leave pmpcfg byte = 0.
            write_pmpaddr(i, region.base >> 2);
        }
    }

    crate::serial::puts("[pmp] Kernel entries locked\n");
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
