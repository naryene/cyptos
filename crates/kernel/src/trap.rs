//! RISC-V trap handling for M-mode
//!
//! Sets up mtvec and handles exceptions/interrupts in machine mode.
//! Reference: RISC-V Privileged Specification v1.12

use core::arch::{asm, naked_asm};

/// Trap frame saved on stack during trap handling.
/// Must match the save/restore order in trap_vector.
#[repr(C)]
pub struct TrapFrame {
    pub ra: u64,
    pub t0: u64,
    pub t1: u64,
    pub t2: u64,
    pub t3: u64,
    pub t4: u64,
    pub t5: u64,
    pub t6: u64,
    pub a0: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
    pub a7: u64,
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
    pub gp: u64,
    pub tp: u64,
    pub sp: u64, // Original sp before trap
}

/// RISC-V exception causes (mcause values when interrupt bit = 0)
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
#[allow(dead_code)]
pub enum Exception {
    InstructionMisaligned = 0,
    InstructionAccessFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadMisaligned = 4,
    LoadAccessFault = 5,
    StoreMisaligned = 6,
    StoreAccessFault = 7,
    EcallFromU = 8,
    EcallFromS = 9,
    EcallFromM = 11,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StorePageFault = 15,
}

/// RISC-V interrupt causes (mcause values when interrupt bit = 1)
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
#[allow(dead_code)]
pub enum Interrupt {
    SupervisorSoftware = 1,
    MachineSoftware = 3,
    SupervisorTimer = 5,
    MachineTimer = 7,
    SupervisorExternal = 9,
    MachineExternal = 11,
}

/// Initialize trap handling by setting mtvec to our trap vector.
/// Uses direct mode (all traps go to the same handler).
pub fn init() {
    unsafe {
        let trap_addr = trap_vector;
        // Mode 0 = Direct: all traps set pc to BASE
        asm!(
            "csrw mtvec, {addr}",
            addr = in(reg) trap_addr,
            options(nostack)
        );
    }
    crate::serial::puts("[trap] mtvec initialized\n");
}

/// Read mcause CSR
#[inline]
pub fn read_mcause() -> u64 {
    let val: u64;
    unsafe {
        asm!("csrr {}, mcause", out(reg) val, options(nostack));
    }
    val
}

/// Read mepc CSR (exception program counter)
#[inline]
pub fn read_mepc() -> u64 {
    let val: u64;
    unsafe {
        asm!("csrr {}, mepc", out(reg) val, options(nostack));
    }
    val
}

/// Write mepc CSR
#[inline]
pub fn write_mepc(val: u64) {
    unsafe {
        asm!("csrw mepc, {}", in(reg) val, options(nostack));
    }
}

/// Read mtval CSR (trap value - faulting address or instruction)
#[inline]
pub fn read_mtval() -> u64 {
    let val: u64;
    unsafe {
        asm!("csrr {}, mtval", out(reg) val, options(nostack));
    }
    val
}

/// Naked trap vector entry point.
/// Saves all registers, calls trap_handler, restores registers, and returns.
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.trap")]
unsafe extern "C" fn trap_vector() {
    naked_asm!(
    // Save original sp to mscratch temporarily
    "csrw mscratch, sp",

    // Allocate trap frame (31 registers * 8 bytes = 248, align to 16 = 256)
    "addi sp, sp, -256",

    // Save all general-purpose registers
    "sd ra,   0(sp)",
    "sd t0,   8(sp)",
    "sd t1,  16(sp)",
    "sd t2,  24(sp)",
    "sd t3,  32(sp)",
    "sd t4,  40(sp)",
    "sd t5,  48(sp)",
    "sd t6,  56(sp)",
    "sd a0,  64(sp)",
    "sd a1,  72(sp)",
    "sd a2,  80(sp)",
    "sd a3,  88(sp)",
    "sd a4,  96(sp)",
    "sd a5, 104(sp)",
    "sd a6, 112(sp)",
    "sd a7, 120(sp)",
    "sd s0, 128(sp)",
    "sd s1, 136(sp)",
    "sd s2, 144(sp)",
    "sd s3, 152(sp)",
    "sd s4, 160(sp)",
    "sd s5, 168(sp)",
    "sd s6, 176(sp)",
    "sd s7, 184(sp)",
    "sd s8, 192(sp)",
    "sd s9, 200(sp)",
    "sd s10, 208(sp)",
    "sd s11, 216(sp)",
    "sd gp, 224(sp)",
    "sd tp, 232(sp)",

    // Save original sp (from mscratch)
    "csrr t0, mscratch",
    "sd t0, 240(sp)",

    // Call trap_handler with trap frame pointer as argument
    "mv a0, sp",
    "call {handler}",

    // Restore all general-purpose registers
    "ld ra,   0(sp)",
    "ld t0,   8(sp)",
    "ld t1,  16(sp)",
    "ld t2,  24(sp)",
    "ld t3,  32(sp)",
    "ld t4,  40(sp)",
    "ld t5,  48(sp)",
    "ld t6,  56(sp)",
    "ld a0,  64(sp)",
    "ld a1,  72(sp)",
    "ld a2,  80(sp)",
    "ld a3,  88(sp)",
    "ld a4,  96(sp)",
    "ld a5, 104(sp)",
    "ld a6, 112(sp)",
    "ld a7, 120(sp)",
    "ld s0, 128(sp)",
    "ld s1, 136(sp)",
    "ld s2, 144(sp)",
    "ld s3, 152(sp)",
    "ld s4, 160(sp)",
    "ld s5, 168(sp)",
    "ld s6, 176(sp)",
    "ld s7, 184(sp)",
    "ld s8, 192(sp)",
    "ld s9, 200(sp)",
    "ld s10, 208(sp)",
    "ld s11, 216(sp)",
    "ld gp, 224(sp)",
    "ld tp, 232(sp)",

    // Restore original sp
    "ld sp, 240(sp)",

    // Return from machine trap
    "mret",

    handler = sym trap_handler,
    )
}

/// Main trap handler called from trap_vector.
/// Dispatches to appropriate handler based on mcause.
#[unsafe(no_mangle)]
extern "C" fn trap_handler(frame: &mut TrapFrame) {
    let mcause = read_mcause();
    let mepc = read_mepc();
    let mtval = read_mtval();

    let is_interrupt = (mcause >> 63) & 1 == 1;
    let cause_code = mcause & 0x7FFF_FFFF_FFFF_FFFF;

    if is_interrupt {
        handle_interrupt(cause_code, frame);
    } else {
        handle_exception(cause_code, mepc, mtval, frame);
    }
}

/// Handle interrupts
fn handle_interrupt(cause: u64, _frame: &mut TrapFrame) {
    match cause {
        7 => {
            // Machine timer interrupt
            crate::timer::increment_tick();
            let ticks = crate::timer::get_tick_count();
            // Print every 100 ticks (1 second at 10ms tick)
            if ticks % 100 == 0 {
                crate::serial::puts("[timer] tick ");
                crate::serial::put_dec(ticks);
                crate::serial::puts("\n");
            }
            crate::timer::schedule_next_tick();
        }
        11 => {
            // Machine external interrupt
            crate::serial::puts("[trap] External interrupt\n");
        }
        3 => {
            // Machine software interrupt
            crate::serial::puts("[trap] Software interrupt\n");
        }
        _ => {
            crate::serial::puts("[trap] Unknown interrupt: ");
            crate::serial::put_dec(cause);
            crate::serial::puts("\n");
        }
    }
}

/// Handle exceptions (synchronous traps)
fn handle_exception(cause: u64, mepc: u64, mtval: u64, _frame: &mut TrapFrame) {
    match cause {
        0 => panic_exception("Instruction address misaligned", mepc, mtval),
        1 => panic_exception("Instruction access fault", mepc, mtval),
        2 => panic_exception("Illegal instruction", mepc, mtval),
        3 => {
            // Breakpoint - advance pc past ebreak and continue
            crate::serial::puts("[trap] Breakpoint at ");
            crate::serial::put_hex(mepc);
            crate::serial::puts("\n");
            write_mepc(mepc + 4); // ebreak is 4 bytes (compressed = 2)
        }
        4 => panic_exception("Load address misaligned", mepc, mtval),
        5 => panic_exception("Load access fault", mepc, mtval),
        6 => panic_exception("Store address misaligned", mepc, mtval),
        7 => panic_exception("Store access fault", mepc, mtval),
        8 => {
            // Environment call from U-mode (syscall)
            crate::serial::puts("[trap] Syscall from U-mode\n");
            // TODO: Implement syscall handling
            write_mepc(mepc + 4);
        }
        9 => {
            // Environment call from S-mode
            crate::serial::puts("[trap] Ecall from S-mode\n");
            write_mepc(mepc + 4);
        }
        11 => {
            // Environment call from M-mode
            crate::serial::puts("[trap] Ecall from M-mode\n");
            write_mepc(mepc + 4);
        }
        12 => panic_exception("Instruction page fault", mepc, mtval),
        13 => panic_exception("Load page fault", mepc, mtval),
        15 => panic_exception("Store page fault", mepc, mtval),
        _ => {
            crate::serial::puts("[trap] Unknown exception: ");
            crate::serial::put_dec(cause);
            crate::serial::puts(" at ");
            crate::serial::put_hex(mepc);
            crate::serial::puts("\n");
            panic!("Unhandled exception");
        }
    }
}

/// Print exception info and panic
fn panic_exception(name: &str, mepc: u64, mtval: u64) -> ! {
    crate::serial::puts("[trap] FATAL: ");
    crate::serial::puts(name);
    crate::serial::puts("\n  mepc:  ");
    crate::serial::put_hex(mepc);
    crate::serial::puts("\n  mtval: ");
    crate::serial::put_hex(mtval);
    crate::serial::puts("\n");
    panic!("{}", name);
}
