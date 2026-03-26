//! Round-robin task scheduler for CyptOS.
//!
//! Manages a static task table and selects the next runnable task on each
//! timer tick. Called from the machine-mode timer ISR in trap.rs.

use super::task::{MAX_TASKS, PmpConfig, Task, TaskId, TaskState};
use crate::pmp::PmpRegion;
use crate::sync::IrqCell;

struct SchedulerState {
    current: usize,
    tasks: [Option<Task>; MAX_TASKS],
}

static SCHEDULER: IrqCell<SchedulerState> = IrqCell::new(SchedulerState {
    current: 0, // Idle task is always slot 0.
    tasks: [None, None, None, None],
});

/// Initialize the scheduler.
///
/// Call from kmain BEFORE enabling interrupts.
/// Transitions all `Created` tasks to `Ready` state so they are eligible
/// for scheduling on the first timer tick.
pub fn init() {
    SCHEDULER.with_lock(|state| {
        #[allow(clippy::needless_range_loop)]
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut state.tasks[i]
                && task.state == TaskState::Created
            {
                task.state = TaskState::Ready;
            }
        }
    });
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
    // Read mepc/mstatus CSRs before entering the lock (they don't need protection).
    let mepc: u64 = crate::arch::csr::mepc::read();
    let mstatus: u64 = crate::arch::csr::mstatus::read();

    // Holds the pmp_regions and context of the next task to switch to.
    // Extracted inside the lock, applied outside.
    let switch_target: Option<(usize, [PmpRegion; 16], super::task::TaskContext)>;

    switch_target = SCHEDULER.with_lock(|state| {
        let current = state.current;

        // --- Step 1: save current task context ---
        if let Some(task) = &mut state.tasks[current] {
            let ctx = &mut task.context;

            // Copy 31 GPRs from TrapFrame into TaskContext.
            crate::arch::register::copy_gpr_fields!(frame, ctx);
            ctx.mepc = mepc;
            ctx.mstatus = mstatus;

            // Preserve Dead state — only transition Running→Ready.
            if task.state == TaskState::Running {
                task.state = TaskState::Ready;
            }
        }

        // --- Step 2: find next Ready task (round-robin) ---
        // Start search at the slot after the current one to ensure fairness.
        let start = (current + 1) % MAX_TASKS;

        let mut next_idx: Option<usize> = None;
        for i in 0..MAX_TASKS {
            let idx = (start + i) % MAX_TASKS;
            if let Some(task) = &state.tasks[idx]
                && task.state == TaskState::Ready
            {
                next_idx = Some(idx);
                break;
            }
        }

        let next_idx = match next_idx {
            Some(idx) => idx,
            None => return None, // No ready task — remain in kernel idle loop.
        };

        // --- Step 3: mark next task as Running and update current ---
        if let Some(task) = &mut state.tasks[next_idx] {
            task.state = TaskState::Running;
        }
        state.current = next_idx;

        // Extract pmp_regions and context to apply outside the lock.
        match &state.tasks[next_idx] {
            Some(task) => Some((next_idx, task.pmp_config.regions, task.context)),
            None => None,
        }
    });

    // Apply the switch outside the lock (frame and CSR writes don't need it).
    if let Some((next_idx, pmp_regions, ctx)) = switch_target {
        crate::serial::puts("[sched] switching to task ");
        crate::serial::put_dec(next_idx as u64);
        crate::serial::puts("\n");

        // --- Step 4: load PMP config for the new task ---
        // PMP must be applied before the TrapFrame is restored so that the
        // new task's address space is active when mret returns to U-mode.
        crate::pmp::load_task_config(&pmp_regions);

        // --- Step 5: copy new task's context into TrapFrame ---
        crate::arch::register::copy_gpr_fields!(ctx, frame);

        // --- Step 6: write new task's mepc and mstatus CSRs ---
        crate::arch::csr::mepc::write(ctx.mepc);
        crate::arch::csr::mstatus::write(ctx.mstatus);
    }
}

/// Create a new task and add it to the task table.
///
/// Returns the `TaskId` on success. Panics if the task table is full.
/// `entry` is the initial program counter (U-mode entry point).
/// `stack_top` is the initial stack pointer.
/// `pmp_regions` is a slice of up to 16 `PmpRegion` descriptors for this task.
pub fn create_task(entry: u64, stack_top: u64, pmp_regions: &[PmpRegion]) -> TaskId {
    SCHEDULER.with_lock(|state| {
        #[allow(clippy::needless_range_loop)]
        for idx in 0..MAX_TASKS {
            if state.tasks[idx].is_none() {
                let mut task = Task::new(TaskId(idx as u8), entry, stack_top);
                for (i, r) in pmp_regions.iter().enumerate().take(16) {
                    task.pmp_config.regions[i] = *r;
                }
                state.tasks[idx] = Some(task);
                return TaskId(idx as u8);
            }
        }
        panic!("task table full");
    })
}

/// Create a new M-mode task (e.g. idle task) with no PMP regions.
///
/// Uses `TaskContext::new_mmode` so `mret` returns to M-mode (MPP=11).
pub fn create_task_mmode(entry: u64, stack_top: u64) -> TaskId {
    SCHEDULER.with_lock(|state| {
        #[allow(clippy::needless_range_loop)]
        for idx in 0..MAX_TASKS {
            if state.tasks[idx].is_none() {
                let mut task = Task::new(TaskId(idx as u8), entry, stack_top);
                task.context = super::task::TaskContext::new_mmode(entry, stack_top);
                state.tasks[idx] = Some(task);
                return TaskId(idx as u8);
            }
        }
        panic!("task table full");
    })
}

/// Kill the currently running task and immediately schedule the next ready task.
///
/// Sets the current task's state to `Dead` so the round-robin loop skips it,
/// then delegates to `schedule` to pick the next `Ready` task. Called from
/// the trap handler when a U-mode access fault is detected.
pub fn kill_current(frame: &mut crate::trap::TrapFrame) {
    SCHEDULER.with_lock(|state| {
        let current = state.current;
        if let Some(task) = &mut state.tasks[current] {
            task.state = TaskState::Dead;
            crate::serial::puts("[sched] task killed: ");
            crate::serial::put_dec(current as u64);
            crate::serial::puts("\n");
        }
    });
    // Schedule the next ready task (or remain idle if none).
    schedule(frame);
}

#[allow(dead_code)]
const fn _empty_pmp_config() -> PmpConfig {
    PmpConfig::empty()
}
