//! Synchronization primitives for bare-metal single-hart RISC-V.
//!
//! `IrqCell<T>` wraps shared mutable state and disables machine interrupts (MIE)
//! during access, preventing re-entrant modification from interrupt handlers.
//! `AtomicCounter` provides a lock-free counter using `AtomicU64` for data that
//! is written from ISR context and read from the main loop (e.g. tick count).
//!
//! Inspired by Linux's `local_irq_save`/`local_irq_restore` for uniprocessor paths
//! and `atomic_t` for simple counters. When multi-hart support is added, `IrqCell`
//! will need to be upgraded to a spinlock.

pub mod atomic_counter;
pub mod irq_cell;
pub use atomic_counter::AtomicCounter;
pub use irq_cell::IrqCell;
