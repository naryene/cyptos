//! RISC-V CLINT (Core Local Interruptor) timer driver
//!
//! CLINT provides machine timer (mtime/mtimecmp) and software interrupts.
//! QEMU virt machine CLINT base: 0x200_0000

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

/// CLINT MMIO base address for QEMU virt
const CLINT_BASE: usize = 0x200_0000;

/// mtime register offset (64-bit read-only counter)
const MTIME_OFFSET: usize = 0xBFF8;

/// mtimecmp register offset for hart 0 (64-bit)
const MTIMECMP_OFFSET: usize = 0x4000;

/// Timer frequency in Hz (QEMU virt: 10 MHz)
const TIMER_FREQ: u64 = 10_000_000;

/// Default tick interval in microseconds
const DEFAULT_TICK_US: u64 = 10_000; // 10ms

/// Read current mtime value
#[inline]
pub fn read_mtime() -> u64 {
    unsafe { read_volatile((CLINT_BASE + MTIME_OFFSET) as *const u64) }
}

/// Write to mtimecmp for hart 0
#[inline]
pub fn write_mtimecmp(val: u64) {
    unsafe { write_volatile((CLINT_BASE + MTIMECMP_OFFSET) as *mut u64, val) }
}

/// Enable machine timer interrupt (set MIE.MTIE)
pub fn enable_timer_interrupt() {
    const MIE_MTIE: u64 = 1 << 7;
    unsafe {
        asm!(
            "csrs mie, {}",
            in(reg) MIE_MTIE,
            options(nostack)
        );
    }
}

/// Disable machine timer interrupt (clear MIE.MTIE)
#[allow(dead_code)]
pub fn disable_timer_interrupt() {
    const MIE_MTIE: u64 = 1 << 7;
    unsafe {
        asm!(
            "csrc mie, {}",
            in(reg) MIE_MTIE,
            options(nostack)
        );
    }
}

/// Enable global machine interrupts (set mstatus.MIE)
pub fn enable_interrupts() {
    const MSTATUS_MIE: u64 = 1 << 3;
    unsafe {
        asm!(
            "csrs mstatus, {}",
            in(reg) MSTATUS_MIE,
            options(nostack)
        );
    }
}

/// Disable global machine interrupts (clear mstatus.MIE)
#[allow(dead_code)]
pub fn disable_interrupts() {
    const MSTATUS_MIE: u64 = 1 << 3;
    unsafe {
        asm!(
            "csrc mstatus, {}",
            in(reg) MSTATUS_MIE,
            options(nostack)
        );
    }
}

/// Convert microseconds to timer ticks
#[inline]
const fn us_to_ticks(us: u64) -> u64 {
    (us * TIMER_FREQ) / 1_000_000
}

/// Set next timer interrupt at current time + delta microseconds
pub fn set_timer_us(delta_us: u64) {
    let current = read_mtime();
    let delta_ticks = us_to_ticks(delta_us);
    write_mtimecmp(current + delta_ticks);
}

/// Clear pending timer interrupt by setting mtimecmp far in the future
pub fn clear_timer() {
    write_mtimecmp(u64::MAX);
}

/// Schedule next tick (called from timer interrupt handler)
pub fn schedule_next_tick() {
    set_timer_us(DEFAULT_TICK_US);
}

static mut TICK_COUNT: u64 = 0;

/// Get current tick count
pub fn get_tick_count() -> u64 {
    unsafe { read_volatile(&raw const TICK_COUNT) }
}

/// Increment tick count (called from timer interrupt handler)
pub fn increment_tick() {
    unsafe {
        TICK_COUNT = TICK_COUNT.wrapping_add(1);
    }
}

/// Initialize timer subsystem
pub fn init() {
    clear_timer();
    enable_timer_interrupt();
    set_timer_us(DEFAULT_TICK_US);

    crate::serial::puts("[timer] CLINT initialized, tick=");
    crate::serial::put_dec(DEFAULT_TICK_US / 1000);
    crate::serial::puts("ms\n");
}
