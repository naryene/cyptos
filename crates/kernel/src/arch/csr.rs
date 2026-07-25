//! RISC-V Control and Status Register (CSR) access layer.
//!
//! Four base macros (`csr_read!`, `csr_write!`, `csr_set_bits!`, `csr_clear_bits!`)
//! map directly to the RISC-V `csrr`, `csrw`, `csrs`, and `csrc` instructions.
//! Typed submodules (`mstatus`, `mcause`, `mepc`, `mtval`, `mtvec`, `mie`) wrap
//! these macros with named constants and helper functions so callers never need
//! to write raw inline assembly for CSR access.

/// Reads a CSR (csrr instruction).
macro_rules! csr_read {
    ($csr:literal) => {{
        let val: u64;
        unsafe { core::arch::asm!(concat!("csrr {}, ", $csr), out(reg) val, options(nostack)) };
        val
    }};
}

/// Writes a CSR (csrw instruction).
macro_rules! csr_write {
    ($csr:literal, $val:expr) => {
        unsafe { core::arch::asm!(concat!("csrw ", $csr, ", {}"), in(reg) $val, options(nostack)) }
    };
}

/// Sets bits in a CSR (csrs instruction).
macro_rules! csr_set_bits {
    ($csr:literal, $bits:expr) => {
        unsafe { core::arch::asm!(concat!("csrs ", $csr, ", {}"), in(reg) $bits, options(nostack)) }
    };
}

/// Clears bits in a CSR (csrc instruction).
macro_rules! csr_clear_bits {
    ($csr:literal, $bits:expr) => {
        unsafe { core::arch::asm!(concat!("csrc ", $csr, ", {}"), in(reg) $bits, options(nostack)) }
    };
}

/// Machine Status register (mstatus): interrupt enable and privilege mode.
pub mod mstatus {
    pub const MIE: u64 = 1 << 3;

    pub fn read() -> u64 {
        csr_read!("mstatus")
    }
    pub fn write(val: u64) {
        csr_write!("mstatus", val)
    }
    pub fn set_bits(bits: u64) {
        csr_set_bits!("mstatus", bits)
    }
    pub fn clear_bits(bits: u64) {
        csr_clear_bits!("mstatus", bits)
    }
    pub fn read_mpp() -> u64 {
        (read() >> 11) & 0b11
    }
}

/// Machine Cause register (mcause): interrupt/exception type and code.
pub mod mcause {
    pub fn read() -> u64 {
        csr_read!("mcause")
    }
}

/// Machine Exception Program Counter (mepc): where exception occurred.
pub mod mepc {
    pub fn read() -> u64 {
        csr_read!("mepc")
    }
    pub fn write(val: u64) {
        csr_write!("mepc", val)
    }
}

/// Machine Trap Value (mtval): bad address or instruction for trap.
pub mod mtval {
    pub fn read() -> u64 {
        csr_read!("mtval")
    }
}

/// Machine Trap Vector (mtvec): interrupt/exception handler address.
pub mod mtvec {
    pub fn write(val: u64) {
        csr_write!("mtvec", val)
    }
}

/// Machine Interrupt Enable (mie): individual interrupt enable bits.
pub mod mie {
    pub const MTIE: u64 = 1 << 7;
    pub fn enable_timer() {
        csr_set_bits!("mie", MTIE)
    }
    pub fn disable_timer() {
        csr_clear_bits!("mie", MTIE)
    }
}
