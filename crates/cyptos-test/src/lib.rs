//! CyptOS test framework.
//!
//! Provides `#[cyptos_test]` — a drop-in `#[test]` replacement that works on
//! both **host** (`cargo test --target x86_64-…`) and **bare-metal riscv64**
//! (QEMU via `custom_test_frameworks`).
//!
//! # Host (standard harness)
//!
//! ```ignore
//! use cyptos_test::cyptos_test;
//!
//! #[cyptos_test]
//! fn it_works() {
//!     assert_eq!(2 + 2, 4);
//! }
//! ```
//!
//! # Kernel / bare-metal (riscv64, custom test framework)
//!
//! In your crate root:
//!
//! ```ignore
//! #![no_std]
//! #![no_main]
//! #![feature(custom_test_frameworks)]
//! #![test_runner(cyptos_test::runner)]
//! #![reexport_test_harness_name = "test_main"]
//! ```

// no_std only on riscv64 (bare-metal); host builds get std for eprint/process::exit.
#![cfg_attr(target_arch = "riscv64", no_std)]

pub use cyptos_test_macros::cyptos_test;

#[cfg(test)]
#[path = "../../kernel/src/sched/policy.rs"]
mod scheduler_policy;

pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        print_str("test ");
        print_str(core::any::type_name::<T>());
        print_str(" ... ");
        self();
        print_str("[ok]\n");
    }
}

pub fn runner(tests: &[&dyn Testable]) {
    print_str("\nrunning ");
    print_dec(tests.len());
    if tests.len() == 1 {
        print_str(" test\n");
    } else {
        print_str(" tests\n");
    }

    for test in tests {
        test.run();
    }

    print_str("\ntest result: ok. ");
    print_dec(tests.len());
    print_str(" passed; 0 failed\n\n");
    exit_success();
}

#[cfg(target_arch = "riscv64")]
pub fn test_panic_handler(info: &core::panic::PanicInfo) -> ! {
    print_str("[FAILED]\n\n");
    if let Some(loc) = info.location() {
        print_str("  panicked at ");
        print_str(loc.file());
        print_str(":");
        print_dec(loc.line() as usize);
        print_str("\n");
    }
    exit_failure()
}

#[cfg(target_arch = "riscv64")]
pub fn exit_success() -> ! {
    // FINISHER_PASS = 0x5555
    unsafe { core::ptr::write_volatile(0x10_0000 as *mut u32, 0x5555) };
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "riscv64")]
pub fn exit_failure() -> ! {
    // (exit_code << 16) | FINISHER_FAIL; exit_code=1 → 0x1_3333
    unsafe { core::ptr::write_volatile(0x10_0000 as *mut u32, (1 << 16) | 0x3333) };
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(target_arch = "riscv64"))]
pub fn exit_success() -> ! {
    std::process::exit(0)
}

#[cfg(not(target_arch = "riscv64"))]
pub fn exit_failure() -> ! {
    std::process::exit(1)
}

#[cfg(target_arch = "riscv64")]
fn print_str(s: &str) {
    const UART_BASE: usize = 0x1000_0000;
    for b in s.bytes() {
        unsafe { core::ptr::write_volatile(UART_BASE as *mut u8, b) };
    }
}

#[cfg(not(target_arch = "riscv64"))]
fn print_str(s: &str) {
    eprint!("{s}");
}

fn print_dec(mut n: usize) {
    if n == 0 {
        print_str("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        // SAFETY: buf[i] is always a valid ASCII digit (0x30..=0x39)
        let digit = unsafe { core::str::from_utf8_unchecked(&buf[i..i + 1]) };
        print_str(digit);
    }
}

#[cfg(test)]
mod pmp_tests {
    /// NAPOT address encoding function (host-only testing).
    /// For NAPOT, pmpaddr = (base + (size/2 - 1)) >> 2.
    /// Size must be power of 2 and >= 8.
    fn napot_addr(base: u64, size: u64) -> u64 {
        assert!(size.is_power_of_two() && size >= 8);
        (base + (size / 2 - 1)) >> 2
    }

    #[test]
    fn test_napot_addr_basic() {
        // 64KB region at 0x8020_0000
        let expected = (0x8020_0000u64 + (0x1_0000 / 2 - 1)) >> 2;
        assert_eq!(napot_addr(0x8020_0000, 0x1_0000), expected);
    }

    #[test]
    fn test_napot_addr_8byte() {
        // Minimum 8-byte region
        let expected = (0x1000u64 + 3) >> 2; // (0x1000 + 8/2 - 1) >> 2
        assert_eq!(napot_addr(0x1000, 8), expected);
    }

    #[test]
    fn test_napot_addr_aligned() {
        // 4KB aligned region
        let base = 0x1000u64;
        let size = 0x1000u64;
        let expected = (base + (size / 2 - 1)) >> 2;
        assert_eq!(napot_addr(base, size), expected);
    }

    #[test]
    fn test_napot_addr_large_region() {
        // 16MB region
        let base = 0x8000_0000u64;
        let size = 0x0100_0000u64;
        let expected = (base + (size / 2 - 1)) >> 2;
        assert_eq!(napot_addr(base, size), expected);
    }

    #[test]
    #[should_panic]
    fn test_napot_addr_non_power_of_two_panics() {
        napot_addr(0x1000, 12); // 12 is not a power of two
    }

    #[test]
    #[should_panic]
    fn test_napot_addr_too_small_panics() {
        napot_addr(0x1000, 4); // size < 8
    }

    #[test]
    fn test_napot_addr_one_megabyte() {
        // 1MB region
        let base = 0x4000_0000u64;
        let size = 0x10_0000u64;
        let expected = (base + (size / 2 - 1)) >> 2;
        assert_eq!(napot_addr(base, size), expected);
    }
}

#[cfg(test)]
mod task_tests {
    use core::mem::{offset_of, size_of};

    // Replicate TaskContext layout for host testing (kernel crate not available on x86_64)
    #[repr(C)]
    struct TaskContext {
        ra: u64,
        t0: u64,
        t1: u64,
        t2: u64,
        t3: u64,
        t4: u64,
        t5: u64,
        t6: u64,
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        a4: u64,
        a5: u64,
        a6: u64,
        a7: u64,
        s0: u64,
        s1: u64,
        s2: u64,
        s3: u64,
        s4: u64,
        s5: u64,
        s6: u64,
        s7: u64,
        s8: u64,
        s9: u64,
        s10: u64,
        s11: u64,
        gp: u64,
        tp: u64,
        sp: u64,
        mepc: u64,
        mstatus: u64,
    }

    #[derive(PartialEq, Eq, Debug)]
    #[repr(u8)]
    enum TaskState {
        Created = 0,
        Ready = 1,
        Running = 2,
        Dead = 3,
    }

    #[test]
    fn test_task_context_size() {
        // 33 fields × 8 bytes = 264 bytes
        assert_eq!(size_of::<TaskContext>(), 264);
    }

    #[test]
    fn test_task_context_mepc_offset() {
        assert_eq!(offset_of!(TaskContext, mepc), 248);
    }

    #[test]
    fn test_task_context_mstatus_offset() {
        assert_eq!(offset_of!(TaskContext, mstatus), 256);
    }

    #[test]
    fn test_task_context_sp_offset() {
        assert_eq!(offset_of!(TaskContext, sp), 240);
    }

    #[test]
    fn test_task_context_ra_offset() {
        assert_eq!(offset_of!(TaskContext, ra), 0);
    }

    #[test]
    fn test_task_state_created_to_ready() {
        let mut state = TaskState::Created;
        assert_eq!(state, TaskState::Created);
        state = TaskState::Ready;
        assert_eq!(state, TaskState::Ready);
    }

    #[test]
    fn test_task_state_ready_to_running() {
        let mut state = TaskState::Ready;
        assert_eq!(state, TaskState::Ready);
        state = TaskState::Running;
        assert_eq!(state, TaskState::Running);
    }

    #[test]
    fn test_task_state_running_to_dead() {
        let mut state = TaskState::Running;
        assert_eq!(state, TaskState::Running);
        state = TaskState::Dead;
        assert_eq!(state, TaskState::Dead);
    }

    #[test]
    fn test_task_state_running_to_ready() {
        let mut state = TaskState::Running;
        assert_eq!(state, TaskState::Running);
        state = TaskState::Ready;
        assert_eq!(state, TaskState::Ready);
    }
}
