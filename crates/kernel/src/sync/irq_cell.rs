//! Interrupt-disable critical section wrapper for shared kernel state.
//!
//! `IrqCell<T>` clears the `mstatus.MIE` bit before granting mutable access
//! and restores it afterward, ensuring that timer ISRs (which invoke the
//! scheduler) cannot preempt code that is modifying the task table.
//! On a single-hart system this is sufficient for mutual exclusion.

use core::arch::asm;
use core::cell::UnsafeCell;

/// Interrupt-safe cell for single-hart bare-metal use.
/// Disables machine interrupts (MIE) during access, preventing
/// re-entrant access from interrupt handlers.
pub struct IrqCell<T> {
    inner: UnsafeCell<T>,
}

// SAFETY: On a single-hart system, disabling interrupts prevents all
// concurrent access. No other hart can observe the inner value.
unsafe impl<T> Sync for IrqCell<T> {}

impl<T> IrqCell<T> {
    pub const fn new(val: T) -> Self {
        Self {
            inner: UnsafeCell::new(val),
        }
    }

    /// Execute `f` with interrupts disabled, then restore previous interrupt state.
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Save mstatus and disable MIE
        let mstatus: u64;
        unsafe { asm!("csrr {}, mstatus", out(reg) mstatus, options(nostack)) };
        unsafe { asm!("csrc mstatus, {}", in(reg) 1u64 << 3, options(nostack)) };

        let result = f(unsafe { &mut *self.inner.get() });

        // Restore previous MIE state (only re-enable if it was enabled before)
        if mstatus & (1 << 3) != 0 {
            unsafe { asm!("csrs mstatus, {}", in(reg) 1u64 << 3, options(nostack)) };
        }

        result
    }
}
