//! CyptOS kernel entry point and boot sequence.
//!
//! `_start` (naked) runs on hart 0: sets the stack pointer, zeroes `.bss`, and
//! calls `kmain`. Secondary harts park in a `wfi` loop. `kmain` initializes the
//! heap, trap vector, PMP, timer, creates user-mode tasks with per-task PMP
//! regions, starts the scheduler, and enters the idle loop. All subsequent
//! scheduling is driven by the timer ISR in `trap.rs`.
#![no_std]
#![no_main]
#![feature(allocator_api)]

extern crate alloc;

#[macro_use]
mod print;

mod allocator;
mod arch;
mod config;
mod pmp;
mod sched;
mod serial;
mod sync;
mod timer;
mod trap;
mod user_task;

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

/// Idle task entry point — runs in M-mode, loops on `wfi`.
///
/// The scheduler falls back to this task when no user task is Ready.
fn idle_task_entry() -> ! {
    loop {
        unsafe { asm!("wfi") };
    }
}

unsafe extern "C" {
    static _stack_top: u8;
    static _sbss: u8;
    static _ebss: u8;
    static _sheap: u8;
    static _eheap: u8;
}

#[global_allocator]
static ALLOCATOR: allocator::BumpAllocator = allocator::BumpAllocator::new();

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

// Helpers
//

fn print_banner() {
    serial::init();
    println!("CyptOS v0.1.0");
    println!("RV64 bare-metal microkernel");
    println!("----------------------------");
}

fn init_heap() {
    // Initialize heap allocator
    let heap_start = &raw const _sheap as usize;
    let heap_end = &raw const _eheap as usize;
    ALLOCATOR.init(heap_start, heap_end);
    serial::puts("[alloc] Heap initialized\n");

    // Verify heap works with a small allocation
    {
        let v: alloc::vec::Vec<u8> = alloc::vec![1u8, 2, 3];
        println!("[alloc] Test alloc ok, len={}", v.len());
    }
}

fn spawn_idle_task() {
    // --- Idle task (M-mode, no PMP, always Ready) ---
    let idle_id = sched::TaskSpec::machine(idle_task_entry).spawn();
    serial::puts("[sched] idle task created: ");
    serial::put_dec(idle_id.0 as u64);
    serial::puts("\n");
}

fn spawn_echo_task(user_text_base: u64) {
    // UART MMIO (RW, 4 KB) — echo task needs UART access via syscall path.
    const MMIO: &[(u64, u64)] = &[(config::UART_BASE as u64, 0x1000)];
    let task0_id =
        sched::TaskSpec::user(user_task::user_echo_task, user_text_base, 0x1000, MMIO).spawn();
    serial::puts("[sched] task created: ");
    serial::put_dec(task0_id.0 as u64);
    serial::puts("\n");
}

fn spawn_violation_task(user_text_base: u64) {
    // No UART entry — violation task intentionally reads kernel memory instead.
    let task1_id =
        sched::TaskSpec::user(user_task::user_violation_task, user_text_base, 0x1000, &[]).spawn();
    serial::puts("[sched] task created: ");
    serial::put_dec(task1_id.0 as u64);
    serial::puts("\n");
}

fn start_scheduler() -> ! {
    sched::init(); // register tasks with the round-robin scheduler

    timer::enable_interrupts(); // unmask mstatus.MIE — scheduler starts firing

    serial::puts("[kernel] Scheduler started — idle task handles wfi\n");
    // The first timer tick dispatches a normal task without saving this
    // bootstrap context. Idle remains available on its dedicated stack.
    // kmain must still not return (it's a diverging function), so we wfi here
    // until the first timer interrupt fires and switches to the idle task.
    loop {
        unsafe { asm!("wfi") };
    }
}
#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    print_banner();

    init_heap();

    trap::init(); // install mtvec handler
    pmp::init(); // lock kernel memory regions
    timer::init(); // arm CLINT and enable timer interrupt

    // Declare linker symbols for user text section bounds.
    unsafe extern "C" {
        static _user_text_start: u8;
    }

    spawn_idle_task();

    // --- Task 0: user_echo_task ---
    // SAFETY: _user_text_start is a valid linker symbol address.
    let user_text_base = &raw const _user_text_start as u64;
    spawn_echo_task(user_text_base);

    // --- Task 1: user_violation_task ---
    spawn_violation_task(user_text_base);

    start_scheduler()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n!!! KERNEL PANIC !!!");
    if let Some(loc) = info.location() {
        println!("at {}:{}", loc.file(), loc.line());
    }
    loop {
        unsafe { asm!("wfi") };
    }
}
