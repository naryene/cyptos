//! Lock-free atomic counter for values updated from interrupt context.
//!
//! Uses `AtomicU64` with relaxed ordering — sufficient because the counter is
//! only incremented by the timer ISR on hart 0 and read by kernel code on the
//! same hart. No cross-hart visibility guarantees are needed.

use core::sync::atomic::{AtomicU64, Ordering};

/// Lock-free counter for use in interrupt handlers.
pub struct AtomicCounter {
    inner: AtomicU64,
}

impl AtomicCounter {
    pub const fn new(val: u64) -> Self {
        Self { inner: AtomicU64::new(val) }
    }

    pub fn increment(&self) -> u64 {
        self.inner.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get(&self) -> u64 {
        self.inner.load(Ordering::Relaxed)
    }
}
