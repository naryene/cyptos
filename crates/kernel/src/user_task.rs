//! Embedded user-mode task code for CyptOS.
//!
//! Functions in this module execute in U-mode after a context switch.
//! They communicate with the kernel exclusively via `ecall` (syscall interface).

use core::arch::asm;

/// SYS_GETC: non-blocking read of one byte from UART. Returns 0xFF if no byte available.
#[allow(dead_code)]
const SYS_GETC: u64 = 0;
/// SYS_PUTC: write one byte to UART. a0 = byte value.
#[allow(dead_code)]
const SYS_PUTC: u64 = 1;

/// Memory violation task entry point — deliberately reads kernel-only memory.
///
/// Placed in `.user_text` so it executes in U-mode. Attempts to load from
/// address `0x8000_0000` (kernel text, RX+L, not accessible from U-mode without
/// a matching per-task PMP entry). The resulting Load Access Fault is caught by
/// the trap handler, which calls `scheduler::kill_current` to terminate this task
/// gracefully and continue scheduling.
///
/// # Safety
/// Must only be entered via context switch (mepc set to this address, MPP=00).
#[unsafe(no_mangle)]
#[unsafe(link_section = ".user_text")]
pub unsafe extern "C" fn user_violation_task() -> ! {
    // Intentionally load from kernel-only address to trigger Load Access Fault.
    // The trap handler will catch this and kill the task gracefully.
    // SAFETY: This fault is intentional; the M-mode trap handler will intercept it.
    let _val: u64;
    unsafe {
        asm!(
            "ld {val}, 0({addr})",
            val = out(reg) _val,
            addr = in(reg) 0x8000_0000u64,
            options(nostack),
        );
    }
    // Unreachable: trap handler kills this task before returning here.
    loop {
        core::hint::spin_loop();
    }
}

/// UART echo task entry point.
///
/// Runs in U-mode (MPP=00). Polls SYS_GETC non-blocking; when a real byte
/// arrives (not 0xFF), echoes it back via SYS_PUTC. Also echoes `\n` after `\r`.
///
/// # Safety
/// Must only be entered via context switch (mepc set to this address, MPP=00).
#[unsafe(no_mangle)]
#[unsafe(link_section = ".user_text")]
pub unsafe extern "C" fn user_echo_task() -> ! {
    loop {
        // SYS_GETC non-blocking: a7=0, returns byte in a0 (0xFF = no data).
        // SAFETY: ecall is the U-mode mechanism for kernel services.
        let byte: u64;
        unsafe {
            asm!(
                "li a7, 0",
                "ecall",
                out("a0") byte,
                out("a7") _,
                options(nostack),
            );
        }

        // 0xFF signals no byte available — yield to scheduler via nop and retry.
        if byte == 0xFF {
            // SAFETY: nop is always safe in U-mode; PMP entry 0 permits execution here.
            unsafe { asm!("nop", options(nostack)) };
            continue;
        }

        // SYS_PUTC: echo the received byte. a7=1, a0=byte.
        // SAFETY: ecall is the U-mode mechanism for kernel services.
        unsafe {
            asm!(
                "li a7, 1",
                "ecall",
                in("a0") byte,
                out("a7") _,
                options(nostack),
            );
        }

        // Echo a newline after carriage return for terminal usability.
        if byte == b'\r' as u64 {
            // SAFETY: ecall is the U-mode mechanism for kernel services.
            unsafe {
                asm!(
                    "li a7, 1",
                    "li a0, 10",
                    "ecall",
                    out("a7") _,
                    out("a0") _,
                    options(nostack),
                );
            }
        }
    }
}
