//! RISC-V trap handling for M-mode
//!
//! Sets up mtvec and handles exceptions/interrupts in machine mode.
//! Reference: RISC-V Privileged Specification v1.12

use core::arch::{asm, naked_asm};

use crate::sched::TaskContext;

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

const _: () = assert!(core::mem::size_of::<TrapFrame>() == 248);

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
    let trap_addr = trap_vector as *const () as u64;
    crate::arch::csr::mtvec::write(trap_addr);
    crate::serial::puts("[trap] mtvec initialized\n");
}

/// Read mcause CSR
#[inline]
pub fn read_mcause() -> u64 {
    crate::arch::csr::mcause::read()
}

/// Read mepc CSR (exception program counter)
#[inline]
pub fn read_mepc() -> u64 {
    crate::arch::csr::mepc::read()
}

/// Write mepc CSR
#[inline]
pub fn write_mepc(val: u64) {
    crate::arch::csr::mepc::write(val)
}

/// Read mtval CSR (trap value - faulting address or instruction)
#[inline]
pub fn read_mtval() -> u64 {
    crate::arch::csr::mtval::read()
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
fn handle_interrupt(cause: u64, frame: &mut TrapFrame) {
    match cause {
        7 => {
            // Machine timer interrupt
            crate::timer::increment_tick();
            let ticks = crate::timer::get_tick_count();
            // Print every 100 ticks (1 second at 10ms tick)
            if ticks.is_multiple_of(100) {
                crate::serial::puts("[timer] tick ");
                crate::serial::put_dec(ticks);
                crate::serial::puts("\n");
            }
            crate::timer::schedule_next_tick();
            // Pass the mutable TrapFrame to the scheduler so it can switch tasks.
            // SAFETY: frame is valid for the duration of the interrupt handler.
            crate::sched::schedule(frame);
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

/// Read mstatus CSR and extract MPP bits [12:11].
/// Returns 0b00 for U-mode, 0b11 for M-mode.
#[inline]
fn read_mpp() -> u64 {
    crate::arch::csr::mstatus::read_mpp()
}

/// Handle a memory access fault (causes 1, 5, 7).
///
/// Checks mstatus.MPP to determine fault origin. U-mode faults are handled
/// gracefully (log + kill task). M-mode faults are fatal (panic).
fn handle_access_fault(name: &str, mepc: u64, mtval: u64, frame: &mut TrapFrame) {
    let mpp = read_mpp();
    if mpp == 0b00 {
        // U-mode access fault — log, kill the faulting task, schedule next.
        crate::serial::puts("[trap] U-mode access fault: ");
        crate::serial::puts(name);
        crate::serial::puts(" at mepc=");
        crate::serial::put_hex(mepc);
        crate::serial::puts(" addr=");
        crate::serial::put_hex(mtval);
        crate::serial::puts("\n");
        crate::sched::kill_current(frame);
    } else {
        // M-mode access fault — kernel bug, panic.
        panic_exception(name, mepc, mtval);
    }
}

/// Handle a U-mode syscall (ecall from U-mode).
/// Dispatches based on syscall number in a7.
fn handle_syscall(frame: &mut TrapFrame) {
    match frame.a7 {
        crate::config::SYS_GETC => {
            // SYS_GETC: non-blocking UART read. Returns 0xFF if no byte available.
            frame.a0 = crate::serial::try_getc().unwrap_or(0xFF) as u64;
        }
        crate::config::SYS_PUTC => {
            // SYS_PUTC: write one byte to UART.
            crate::serial::putc(frame.a0 as u8);
        }
        crate::config::SYS_LEGACY_POC => {
            // Legacy POC syscall — kept for compatibility.
            crate::serial::puts("[trap] U-mode POC ecall (legacy)\n");
        }
        _ => {
            crate::serial::puts("[syscall] unknown: ");
            crate::serial::put_dec(frame.a7);
            crate::serial::puts("\n");
        }
    }
}

/// Handle exceptions (synchronous traps)
fn handle_exception(cause: u64, mepc: u64, mtval: u64, frame: &mut TrapFrame) {
    match cause {
        0 => panic_exception("Instruction address misaligned", mepc, mtval),
        1 => handle_access_fault("Instruction access fault", mepc, mtval, frame),
        2 => panic_exception("Illegal instruction", mepc, mtval),
        3 => {
            // Breakpoint - advance pc past ebreak and continue
            crate::serial::puts("[trap] Breakpoint at ");
            crate::serial::put_hex(mepc);
            crate::serial::puts("\n");
            write_mepc(mepc + 4); // ebreak is 4 bytes (compressed = 2)
        }
        4 => panic_exception("Load address misaligned", mepc, mtval),
        5 => handle_access_fault("Load access fault", mepc, mtval, frame),
        6 => panic_exception("Store address misaligned", mepc, mtval),
        7 => handle_access_fault("Store access fault", mepc, mtval, frame),
        8 => {
            handle_syscall(frame);
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

/// Switch CPU context from `old` task to `new` task.
///
/// Saves the current mepc/mstatus CSRs into `old`, then delegates GPR
/// save/restore and the final `mret` to the naked inner helper
/// `do_context_switch_asm`.
///
/// The caller (scheduler) is responsible for installing the new task's PMP
/// configuration via `pmp::load_task_config` **before** calling this function.
///
/// # Safety
/// - `old` must be a valid, aligned pointer to a `TaskContext` for the current task.
/// - `new` must be a valid, aligned pointer to a `TaskContext` for the next task.
/// - Must be called with interrupts disabled (MIE=0); this is the caller's responsibility.
/// - Must NOT be called from U-mode.
#[allow(dead_code)]
pub unsafe fn context_switch(old: *mut TaskContext, new: *const TaskContext) {
    // Save mepc and mstatus CSRs into the old context before the GPR switch.
    // SAFETY: CSR reads are valid in M-mode; `old` is a valid TaskContext pointer.
    unsafe {
        let mepc: u64;
        let mstatus: u64;
        asm!("csrr {}, mepc",    out(reg) mepc,    options(nostack));
        asm!("csrr {}, mstatus", out(reg) mstatus, options(nostack));
        (*old).mepc = mepc;
        (*old).mstatus = mstatus;
    }
    // Perform the GPR save/restore and `mret` via the naked assembly helper.
    // SAFETY: `old` and `new` are valid, aligned TaskContext pointers.
    unsafe { do_context_switch_asm(old, new) }
}

/// Naked inner helper that saves/restores all 31 GPRs and executes `mret`.
///
/// On entry (C-ABI): a0 = old (*mut TaskContext), a1 = new (*const TaskContext).
///
/// Saves all 31 GPRs to `*old`, writes new task's mepc/mstatus to CSRs,
/// loads all 31 GPRs from `*new` (a1 loaded last to exhaust the base pointer),
/// then executes `mret` to transfer control to the new task.
///
/// # Safety
/// Must only be called from `context_switch`. Both pointers must be valid.
#[unsafe(naked)]
unsafe extern "C" fn do_context_switch_asm(_old: *mut TaskContext, _new: *const TaskContext) {
    naked_asm!(
        // --- Save all 31 GPRs to old (a0) ---
        "sd ra,    0(a0)",
        "sd t0,    8(a0)",
        "sd t1,   16(a0)",
        "sd t2,   24(a0)",
        "sd t3,   32(a0)",
        "sd t4,   40(a0)",
        "sd t5,   48(a0)",
        "sd t6,   56(a0)",
        // a0 holds the `old` pointer; saving it as-is is intentional —
        // the real task a0 was captured in the TrapFrame by trap_vector.
        "sd a0,   64(a0)",
        "sd a1,   72(a0)",
        "sd a2,   80(a0)",
        "sd a3,   88(a0)",
        "sd a4,   96(a0)",
        "sd a5,  104(a0)",
        "sd a6,  112(a0)",
        "sd a7,  120(a0)",
        "sd s0,  128(a0)",
        "sd s1,  136(a0)",
        "sd s2,  144(a0)",
        "sd s3,  152(a0)",
        "sd s4,  160(a0)",
        "sd s5,  168(a0)",
        "sd s6,  176(a0)",
        "sd s7,  184(a0)",
        "sd s8,  192(a0)",
        "sd s9,  200(a0)",
        "sd s10, 208(a0)",
        "sd s11, 216(a0)",
        "sd gp,  224(a0)",
        "sd tp,  232(a0)",
        "sd sp,  240(a0)",
        // --- Load new task's CSRs from new (a1), using t0 as scratch ---
        "ld t0,  248(a1)",
        "csrw mepc, t0",
        "ld t0,  256(a1)",
        "csrw mstatus, t0",
        // --- Load all 31 GPRs from new (a1); a1 loaded last ---
        "ld ra,    0(a1)",
        "ld t0,    8(a1)",
        "ld t1,   16(a1)",
        "ld t2,   24(a1)",
        "ld t3,   32(a1)",
        "ld t4,   40(a1)",
        "ld t5,   48(a1)",
        "ld t6,   56(a1)",
        "ld a0,   64(a1)",
        // a1 is still the base pointer; loaded last below
        "ld a2,   80(a1)",
        "ld a3,   88(a1)",
        "ld a4,   96(a1)",
        "ld a5,  104(a1)",
        "ld a6,  112(a1)",
        "ld a7,  120(a1)",
        "ld s0,  128(a1)",
        "ld s1,  136(a1)",
        "ld s2,  144(a1)",
        "ld s3,  152(a1)",
        "ld s4,  160(a1)",
        "ld s5,  168(a1)",
        "ld s6,  176(a1)",
        "ld s7,  184(a1)",
        "ld s8,  192(a1)",
        "ld s9,  200(a1)",
        "ld s10, 208(a1)",
        "ld s11, 216(a1)",
        "ld gp,  224(a1)",
        "ld tp,  232(a1)",
        "ld sp,  240(a1)",
        "ld a1,   72(a1)", // a1 is the base pointer; must be loaded last
        "mret",
    )
}
