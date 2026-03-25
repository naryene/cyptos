#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod pmp;
mod scheduler;
mod serial;
mod task;
mod timer;
mod trap;
mod user_task;

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

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

    trap::init();
    pmp::init();
    timer::init();

    // Declare linker symbols for user text section bounds.
    unsafe extern "C" {
        static _user_text_start: u8;
    }

    // SAFETY: Layout is valid (8 KB size, 8 KB alignment ensures NAPOT-aligned stack).
    let stack_layout = unsafe { alloc::alloc::Layout::from_size_align_unchecked(8192, 8192) };

    // --- Task 0: user_echo_task ---
    // SAFETY: Layout is non-zero-sized; allocator is initialized above.
    let stack0_bottom = unsafe { alloc::alloc::alloc(stack_layout) } as u64;
    let stack0_top = stack0_bottom + 8192;

    // SAFETY: _user_text_start is a valid linker symbol address.
    let user_text_base = &raw const _user_text_start as u64;

    let task0_id = scheduler::create_task(
        user_task::user_echo_task as *const () as u64,
        stack0_top,
        &{
            let mut regions = [pmp::PmpRegion::new(0, 0, 0); 16];
            // Entry 4: user code section (RX, 4 KB, NAPOT).
            regions[4] = pmp::PmpRegion::new(user_text_base, 0x0000_1000, pmp::flags::RX);
            // Entry 5: user stack (RW, 8 KB, NAPOT, 8 KB aligned by stack_layout).
            regions[5] = pmp::PmpRegion::new(stack0_bottom, 0x0000_2000, pmp::flags::RW);
            // Entry 6: UART MMIO (RW, 4 KB) — echo task needs UART access via syscall path.
            regions[6] = pmp::PmpRegion::new(0x1000_0000, 0x0000_1000, pmp::flags::RW);
            regions
        },
    );
    serial::puts("[sched] task created: ");
    serial::put_dec(task0_id.0 as u64);
    serial::puts("\n");

    // --- Task 1: user_violation_task ---
    // SAFETY: Layout is non-zero-sized; allocator is initialized above.
    let stack1_bottom = unsafe { alloc::alloc::alloc(stack_layout) } as u64;
    let stack1_top = stack1_bottom + 8192;

    let task1_id = scheduler::create_task(
        user_task::user_violation_task as *const () as u64,
        stack1_top,
        &{
            let mut regions = [pmp::PmpRegion::new(0, 0, 0); 16];
            // Entry 4: user code section (RX, 4 KB) — violation task runs from user_text.
            regions[4] = pmp::PmpRegion::new(user_text_base, 0x0000_1000, pmp::flags::RX);
            // Entry 5: user stack (RW, 8 KB, NAPOT, 8 KB aligned by stack_layout).
            regions[5] = pmp::PmpRegion::new(stack1_bottom, 0x0000_2000, pmp::flags::RW);
            // No UART entry — violation task intentionally reads kernel memory instead.
            regions
        },
    );
    serial::puts("[sched] task created: ");
    serial::put_dec(task1_id.0 as u64);
    serial::puts("\n");

    scheduler::init();

    timer::enable_interrupts();

    serial::puts("[kernel] Scheduler started, entering idle loop\n");
    loop {
        // SAFETY: wfi is safe in M-mode.
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
