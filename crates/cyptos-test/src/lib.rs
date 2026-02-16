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
