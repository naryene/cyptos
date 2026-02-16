#![no_std]
#![no_main]

mod pmp;
mod serial;
mod timer;
mod trap;

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

unsafe extern "C" {
    static _stack_top: u8;
    static _sbss: u8;
    static _ebss: u8;
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // Read this hart's ID from the mhartid CSR into t0
        "csrr t0, mhartid",
        // If hart ID != 0, skip boot and go straight to the parking loop
        "bnez t0, 2f",
        // Hart 0 only: set up the stack pointer
        "la sp, _stack_top",
        // Zero the .bss section before any Rust code runs
        "call {clear_bss}",
        // Enter the Rust kernel entry point (never returns)
        "call {kmain}",
        // Parking loop: secondary harts (and hart 0 if kmain returns) sleep here
        "2:",
        "wfi",
        // Loop back to wfi in case of spurious wakeups
        "j 2b",
        clear_bss = sym clear_bss,
        kmain = sym kmain,
    )
}

#[unsafe(no_mangle)]
extern "C" fn clear_bss() {
    unsafe {
        let mut ptr = &raw const _sbss as *mut u8;
        let end = &raw const _ebss as *mut u8;
        while ptr < end {
            ptr.write_volatile(0);
            ptr = ptr.add(1);
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    serial::init();
    serial::puts("CyptOS v0.1.0\n");
    serial::puts("RV64 bare-metal microkernel\n");
    serial::puts("----------------------------\n");

    trap::init();
    pmp::init();
    timer::init();

    timer::enable_interrupts();

    serial::puts("[kernel] Boot complete, entering idle loop\n");

    loop {
        unsafe { asm!("wfi") };
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial::puts("\n!!! KERNEL PANIC !!!\n");
    if let Some(loc) = info.location() {
        serial::puts("at ");
        serial::puts(loc.file());
        serial::puts(":");
        serial::put_dec(loc.line() as u64);
        serial::puts("\n");
    }
    loop {
        unsafe { asm!("wfi") };
    }
}
