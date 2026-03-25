//! Round-robin task scheduler for CyptOS.
//!
//! Manages a static task table and selects the next runnable task on each
//! timer tick. Called from the machine-mode timer ISR in trap.rs.

use core::arch::asm;

use crate::pmp::PmpRegion;
use crate::task::{MAX_TASKS, PmpConfig, Task, TaskId, TaskState};

/// Index of the currently executing task. `usize::MAX` = no task running (kernel idle).
static mut CURRENT_TASK: usize = usize::MAX;

/// Static task table. Populated by `create_task` at boot.
static mut TASKS: [Option<Task>; MAX_TASKS] = [None, None, None, None];

/// Initialize the scheduler.
///
/// Call from kmain BEFORE enabling interrupts.
/// Transitions all `Created` tasks to `Ready` state so they are eligible
/// for scheduling on the first timer tick.
pub fn init() {
    // SAFETY: single-hart, called before interrupts are enabled; no concurrent access.
    #[allow(clippy::needless_range_loop)]
    for i in 0..MAX_TASKS {
        let slot: *mut Option<Task> = unsafe { &raw mut TASKS[i] };
        // Raw pointer avoids the Rust 2024 static_mut_refs lint.
        if let Some(task) = unsafe { &mut *slot }
            && task.state == TaskState::Created
        {
            task.state = TaskState::Ready;
        }
    }
}

/// Called from timer ISR on each tick.
///
/// Saves the current task's CPU context from the trap frame and CSRs, selects
/// the next `Ready` task via round-robin, swaps the PMP configuration, and
/// restores the new task's context into the trap frame so that `mret` in
/// `trap_vector` resumes the chosen task.
///
/// `frame`: the `TrapFrame` currently on the kernel stack, saved by `trap_vector`.
///
/// If no ready task is found the function returns without modification (kernel
/// stays in the idle loop).
pub fn schedule(frame: &mut crate::trap::TrapFrame) {
    // --- Step 1: save current task context if one is running ---
    // SAFETY: single-hart bare-metal; TASKS/CURRENT_TASK accessed only from M-mode
    // trap handler which is non-reentrant on this hart. Raw pointers avoid the
    // Rust 2024 static_mut_refs lint.
    let current = unsafe { CURRENT_TASK };
    if current != usize::MAX {
        let slot: *mut Option<Task> = unsafe { &raw mut TASKS[current] };
        // SAFETY: index `current` is always a valid TASKS index when not usize::MAX.
        if let Some(task) = unsafe { &mut *slot } {
            let ctx = &mut task.context;

            // Copy 31 GPRs from TrapFrame into TaskContext.
            ctx.ra = frame.ra;
            ctx.t0 = frame.t0;
            ctx.t1 = frame.t1;
            ctx.t2 = frame.t2;
            ctx.t3 = frame.t3;
            ctx.t4 = frame.t4;
            ctx.t5 = frame.t5;
            ctx.t6 = frame.t6;
            ctx.a0 = frame.a0;
            ctx.a1 = frame.a1;
            ctx.a2 = frame.a2;
            ctx.a3 = frame.a3;
            ctx.a4 = frame.a4;
            ctx.a5 = frame.a5;
            ctx.a6 = frame.a6;
            ctx.a7 = frame.a7;
            ctx.s0 = frame.s0;
            ctx.s1 = frame.s1;
            ctx.s2 = frame.s2;
            ctx.s3 = frame.s3;
            ctx.s4 = frame.s4;
            ctx.s5 = frame.s5;
            ctx.s6 = frame.s6;
            ctx.s7 = frame.s7;
            ctx.s8 = frame.s8;
            ctx.s9 = frame.s9;
            ctx.s10 = frame.s10;
            ctx.s11 = frame.s11;
            ctx.gp = frame.gp;
            ctx.tp = frame.tp;
            ctx.sp = frame.sp;

            // Save mepc and mstatus CSRs — not in TrapFrame.
            // SAFETY: CSR reads are valid in M-mode.
            let mepc: u64;
            let mstatus: u64;
            unsafe {
                asm!("csrr {v}, mepc",    v = out(reg) mepc,    options(nostack));
                asm!("csrr {v}, mstatus", v = out(reg) mstatus, options(nostack));
            }
            ctx.mepc = mepc;
            ctx.mstatus = mstatus;

            // Preserve Dead state — only transition Running→Ready.
            if task.state == TaskState::Running {
                task.state = TaskState::Ready;
            }
        }
    }

    // --- Step 2: find next Ready task (round-robin) ---
    let start = if current == usize::MAX {
        0
    } else {
        (current + 1) % MAX_TASKS
    };

    let mut next_idx: Option<usize> = None;
    for i in 0..MAX_TASKS {
        let idx = (start + i) % MAX_TASKS;
        let slot: *const Option<Task> = unsafe { &raw const TASKS[idx] };
        // SAFETY: idx is always a valid TASKS index (0..MAX_TASKS).
        if let Some(task) = unsafe { &*slot }
            && task.state == TaskState::Ready
        {
            next_idx = Some(idx);
            break;
        }
    }

    let next_idx = match next_idx {
        Some(idx) => idx,
        None => return, // No ready task — remain in kernel idle loop.
    };

    // --- Step 3: mark next task as Running and update CURRENT_TASK ---
    {
        let slot: *mut Option<Task> = unsafe { &raw mut TASKS[next_idx] };
        // SAFETY: next_idx is a valid TASKS index verified by the scan above.
        if let Some(task) = unsafe { &mut *slot } {
            task.state = TaskState::Running;
        }
    }
    // SAFETY: single-hart write; no concurrent access.
    unsafe { CURRENT_TASK = next_idx };

    crate::serial::puts("[sched] switching to task ");
    crate::serial::put_dec(next_idx as u64);
    crate::serial::puts("\n");

    // --- Step 4: load PMP config for the new task (before touching TrapFrame) ---
    // Copy regions to a local to avoid aliasing through the static.
    let pmp_regions: [PmpRegion; 16] = {
        let slot: *const Option<Task> = unsafe { &raw const TASKS[next_idx] };
        // SAFETY: next_idx is a valid TASKS index.
        match unsafe { &*slot } {
            Some(task) => task.pmp_config.regions,
            None => return,
        }
    };
    crate::pmp::load_task_config(&pmp_regions);

    // --- Step 5: copy new task's context into TrapFrame ---
    let ctx = {
        let slot: *const Option<Task> = unsafe { &raw const TASKS[next_idx] };
        // SAFETY: next_idx is a valid TASKS index.
        match unsafe { &*slot } {
            Some(task) => task.context,
            None => return,
        }
    };

    frame.ra = ctx.ra;
    frame.t0 = ctx.t0;
    frame.t1 = ctx.t1;
    frame.t2 = ctx.t2;
    frame.t3 = ctx.t3;
    frame.t4 = ctx.t4;
    frame.t5 = ctx.t5;
    frame.t6 = ctx.t6;
    frame.a0 = ctx.a0;
    frame.a1 = ctx.a1;
    frame.a2 = ctx.a2;
    frame.a3 = ctx.a3;
    frame.a4 = ctx.a4;
    frame.a5 = ctx.a5;
    frame.a6 = ctx.a6;
    frame.a7 = ctx.a7;
    frame.s0 = ctx.s0;
    frame.s1 = ctx.s1;
    frame.s2 = ctx.s2;
    frame.s3 = ctx.s3;
    frame.s4 = ctx.s4;
    frame.s5 = ctx.s5;
    frame.s6 = ctx.s6;
    frame.s7 = ctx.s7;
    frame.s8 = ctx.s8;
    frame.s9 = ctx.s9;
    frame.s10 = ctx.s10;
    frame.s11 = ctx.s11;
    frame.gp = ctx.gp;
    frame.tp = ctx.tp;
    frame.sp = ctx.sp;

    // --- Step 6: write new task's mepc and mstatus CSRs ---
    // trap_vector will execute mret which uses these CSRs to resume the task.
    // SAFETY: CSR writes are valid in M-mode; values come from a verified TaskContext.
    unsafe {
        asm!("csrw mepc,    {v}", v = in(reg) ctx.mepc,    options(nostack));
        asm!("csrw mstatus, {v}", v = in(reg) ctx.mstatus, options(nostack));
    }
}

/// Create a new task and add it to the task table.
///
/// Returns the `TaskId` on success. Panics if the task table is full.
/// `entry` is the initial program counter (U-mode entry point).
/// `stack_top` is the initial stack pointer.
/// `pmp_regions` is a slice of up to 16 `PmpRegion` descriptors for this task.
pub fn create_task(entry: u64, stack_top: u64, pmp_regions: &[PmpRegion]) -> TaskId {
    // SAFETY: single-hart, called from kmain before interrupts are enabled;
    // no concurrent access to TASKS is possible.
    #[allow(clippy::needless_range_loop)]
    for idx in 0..MAX_TASKS {
        let slot: *mut Option<Task> = unsafe { &raw mut TASKS[idx] };
        // Raw pointer avoids Rust 2024 static_mut_refs lint.
        if unsafe { (*slot).is_none() } {
            let mut task = Task::new(TaskId(idx as u8), entry, stack_top);
            for (i, r) in pmp_regions.iter().enumerate().take(16) {
                task.pmp_config.regions[i] = *r;
            }
            // SAFETY: slot points to a valid, currently-None array element.
            unsafe { *slot = Some(task) };
            return TaskId(idx as u8);
        }
    }
    panic!("task table full");
}

/// Kill the currently running task and immediately schedule the next ready task.
///
/// Sets the current task's state to `Dead` so the round-robin loop skips it,
/// then delegates to `schedule` to pick the next `Ready` task. Called from
/// the trap handler when a U-mode access fault is detected.
pub fn kill_current(frame: &mut crate::trap::TrapFrame) {
    // SAFETY: single-hart bare-metal; TASKS/CURRENT_TASK accessed only from
    // M-mode trap handler which is non-reentrant. Raw pointer avoids the
    // Rust 2024 static_mut_refs lint.
    let current = unsafe { CURRENT_TASK };
    if current != usize::MAX {
        let slot: *mut Option<Task> = unsafe { &raw mut TASKS[current] };
        // SAFETY: `current` is a valid TASKS index when not usize::MAX.
        if let Some(task) = unsafe { &mut *slot } {
            task.state = TaskState::Dead;
            crate::serial::puts("[sched] task killed: ");
            crate::serial::put_dec(current as u64);
            crate::serial::puts("\n");
        }
    }
    // Schedule the next ready task (or remain idle if none).
    schedule(frame);
}

#[allow(dead_code)]
const fn _empty_pmp_config() -> PmpConfig {
    PmpConfig::empty()
}
