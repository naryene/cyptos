//! RISC-V CLINT (Core Local Interruptor) timer driver
//!
//! CLINT provides machine timer (mtime/mtimecmp) and software interrupts.
//! QEMU virt machine CLINT base: 0x200_0000

use core::ptr::{read_volatile, write_volatile};

use crate::config::{CLINT_BASE, MTIME_OFFSET, MTIMECMP_OFFSET, TICK_INTERVAL_US, TIMER_FREQ_HZ};
use crate::sync::AtomicCounter;

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
    crate::arch::csr::mie::enable_timer()
}

/// Disable machine timer interrupt (clear MIE.MTIE)
#[allow(dead_code)]
pub fn disable_timer_interrupt() {
    crate::arch::csr::mie::disable_timer()
}

/// Enable global machine interrupts (set mstatus.MIE)
pub fn enable_interrupts() {
    crate::arch::csr::mstatus::set_bits(crate::arch::csr::mstatus::MIE)
}

/// Disable global machine interrupts (clear mstatus.MIE)
#[allow(dead_code)]
pub fn disable_interrupts() {
    crate::arch::csr::mstatus::clear_bits(crate::arch::csr::mstatus::MIE)
}

/// Convert microseconds to timer ticks
#[inline]
const fn us_to_ticks(us: u64) -> u64 {
    (us * TIMER_FREQ_HZ) / 1_000_000
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
    set_timer_us(TICK_INTERVAL_US);
}

static TICK_COUNT: AtomicCounter = AtomicCounter::new(0);

/// Get current tick count
pub fn get_tick_count() -> u64 {
    TICK_COUNT.get()
}

/// Increment tick count (called from timer interrupt handler)
pub fn increment_tick() {
    TICK_COUNT.increment();
}

/// Initialize timer subsystem
pub fn init() {
    clear_timer();
    enable_timer_interrupt();
    set_timer_us(TICK_INTERVAL_US);

    crate::serial::puts("[timer] CLINT initialized, tick=");
    crate::serial::put_dec(TICK_INTERVAL_US / 1000);
    crate::serial::puts("ms\n");
}
