//! Centralized hardware and kernel constants for CyptOS.
//!
//! All MMIO base addresses, timer frequencies, scheduler limits, PMP entry counts,
//! and syscall numbers are defined here so that changes propagate to every module.
//! Values are specific to the QEMU `virt` machine (RV64GC, CLINT at 0x200_0000,
//! NS16550A UART at 0x1000_0000, 10 MHz timer).

// --- MMIO addresses ---
pub const UART_BASE: usize = 0x1000_0000;
pub const CLINT_BASE: usize = 0x0200_0000;

// --- CLINT register offsets ---
pub const MTIME_OFFSET: usize = 0xBFF8;
pub const MTIMECMP_OFFSET: usize = 0x4000;

// --- Timer ---
pub const TIMER_FREQ_HZ: u64 = 10_000_000;
pub const TICK_INTERVAL_US: u64 = 10_000;

// --- Scheduler ---
pub const MAX_TASKS: usize = 4;

// --- Task stack ---
pub const TASK_STACK_SIZE: usize = 8192;
pub const TASK_STACK_ALIGN: usize = 8192;

const _: () = assert!(
    TASK_STACK_SIZE >= 8 && TASK_STACK_SIZE.is_power_of_two(),
    "task stack size must be PMP-NAPOT-compatible"
);
const _: () = assert!(
    TASK_STACK_ALIGN.is_power_of_two() && TASK_STACK_ALIGN >= TASK_STACK_SIZE,
    "task stack alignment must be a power of two and at least the stack size"
);

// --- PMP ---
pub const PMP_COUNT: usize = 16;

// --- Syscall numbers ---
pub const SYS_GETC: u64 = 0;
pub const SYS_PUTC: u64 = 1;
pub const SYS_LEGACY_POC: u64 = 42;
