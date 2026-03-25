//! RISC-V architecture abstractions for M-mode kernel code.
//!
//! Provides safe, typed access to Control and Status Registers (CSRs) and
//! macros for bulk general-purpose register save/restore during context switches.
//! All direct `asm!` CSR manipulation is confined to this module — the rest of
//! the kernel uses the typed wrappers (e.g. `csr::mepc::read()`).

pub mod csr;
pub mod register;
