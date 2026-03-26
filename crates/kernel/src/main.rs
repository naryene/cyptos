//! CyptOS kernel entry point and boot sequence.
//!
//! `_start` (naked) runs on hart 0: sets the stack pointer, zeroes `.bss`, and
//! calls `kmain`. Secondary harts park in a `wfi` loop. `kmain` initializes the
//! heap, trap vector, PMP, timer, creates user-mode tasks with per-task PMP
//! regions, starts the scheduler, and enters the idle loop. All subsequent
//! scheduling is driven by the timer ISR in `trap.rs`.
#![no_std]
#![no_main]

extern crate alloc;

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

#[unsafe(no_mangle)]
extern "C" fn kmain() -> ! {
    serial::init();
    serial::puts("CyptOS v0.1.0\n");
    serial::puts("RV64 bare-metal microkernel\n");
    serial::puts("----------------------------\n");

    // Initialize heap allocator
    let heap_start = &raw const _sheap as usize;
    let heap_end = &raw const _eheap as usize;
    ALLOCATOR.init(heap_start, heap_end);
    serial::puts("[alloc] Heap initialized\n");

    // Verify heap works with a small allocation
    {
        let v: alloc::vec::Vec<u8> = alloc::vec![1u8, 2, 3];
        serial::puts("[alloc] Test alloc ok, len=");
        serial::put_dec(v.len() as u64);
        serial::puts("\n");
    }

    trap::init(); // install mtvec handler
    pmp::init(); // lock kernel memory regions
    timer::init(); // arm CLINT and enable timer interrupt

    // Declare linker symbols for user text section bounds.
    unsafe extern "C" {
        static _user_text_start: u8;
    }

    // SAFETY: Layout is valid (8 KB size, 8 KB alignment ensures NAPOT-aligned stack).
    let stack_layout = unsafe {
        alloc::alloc::Layout::from_size_align_unchecked(
            config::TASK_STACK_SIZE,
            config::TASK_STACK_ALIGN,
        )
    };

    // --- Idle task (M-mode, no PMP, always Ready) ---
    let idle_stack_bottom = unsafe { alloc::alloc::alloc(stack_layout) } as u64;
    let idle_stack_top = idle_stack_bottom + config::TASK_STACK_SIZE as u64;

    let idle_id = sched::create_task_mmode(idle_task_entry as *const () as u64, idle_stack_top);
    serial::puts("[sched] idle task created: ");
    serial::put_dec(idle_id.0 as u64);
    serial::puts("\n");

    // --- Task 0: user_echo_task ---
    // SAFETY: Layout is non-zero-sized; allocator is initialized above.
    let stack0_bottom = unsafe { alloc::alloc::alloc(stack_layout) } as u64;
    let stack0_top = stack0_bottom + config::TASK_STACK_SIZE as u64;

    // SAFETY: _user_text_start is a valid linker symbol address.
    let user_text_base = &raw const _user_text_start as u64;

    let task0_pmp = sched::PmpConfig::builder()
        .code_region(user_text_base, 0x1000)
        .stack_region(stack0_bottom, 0x2000)
        // UART MMIO (RW, 4 KB) — echo task needs UART access via syscall path.
        .mmio_region(config::UART_BASE as u64, 0x1000)
        .build();

    let task0_id = sched::create_task(
        user_task::user_echo_task as *const () as u64,
        stack0_top,
        &task0_pmp.regions,
    );
    serial::puts("[sched] task created: ");
    serial::put_dec(task0_id.0 as u64);
    serial::puts("\n");

    // --- Task 1: user_violation_task ---
    // SAFETY: Layout is non-zero-sized; allocator is initialized above.
    let stack1_bottom = unsafe { alloc::alloc::alloc(stack_layout) } as u64;
    let stack1_top = stack1_bottom + config::TASK_STACK_SIZE as u64;

    let task1_pmp = sched::PmpConfig::builder()
        .code_region(user_text_base, 0x1000)
        // No UART entry — violation task intentionally reads kernel memory instead.
        .stack_region(stack1_bottom, 0x2000)
        .build();

    let task1_id = sched::create_task(
        user_task::user_violation_task as *const () as u64,
        stack1_top,
        &task1_pmp.regions,
    );
    serial::puts("[sched] task created: ");
    serial::put_dec(task1_id.0 as u64);
    serial::puts("\n");

    sched::init(); // register tasks with the round-robin scheduler

    timer::enable_interrupts(); // unmask mstatus.MIE — scheduler starts firing

    serial::puts("[kernel] Scheduler started — idle task handles wfi\n");
    // The idle task (slot 0) now handles the wfi loop.
    // The first timer tick will invoke the scheduler and pick a Ready task.
    // kmain must still not return (it's a diverging function), so we wfi here
    // until the first timer interrupt fires and switches to the idle task.
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
