use super::{PmpConfig, TaskId};
use alloc::alloc::{Allocator, Global};
use core::alloc::Layout;

/// Complete description of a task that can be registered with the scheduler.
///
/// Typed constructors keep machine-mode and user-mode entry points distinct.
/// Spawning owns the common stack allocation and user-task PMP setup.
pub struct TaskSpec {
    entry: u64,
    mode: TaskMode,
}

/// Privilege mode and address-space configuration for a task.
enum TaskMode {
    /// Machine-mode task (idle and future kernel threads). No task PMP entries.
    MMode,
    /// User-mode task with executable code and optional MMIO regions.
    UMode {
        code_base: u64,
        code_size: u64,
        mmio: &'static [(u64, u64)],
    },
}

impl TaskSpec {
    /// Describe a machine-mode task.
    pub fn machine(entry: fn() -> !) -> Self {
        Self {
            entry: entry as usize as u64,
            mode: TaskMode::MMode,
        }
    }

    /// Describe a user-mode task.
    pub fn user(
        entry: unsafe extern "C" fn() -> !,
        code_base: u64,
        code_size: u64,
        mmio: &'static [(u64, u64)],
    ) -> Self {
        Self {
            entry: entry as usize as u64,
            mode: TaskMode::UMode {
                code_base,
                code_size,
                mmio,
            },
        }
    }

    /// Allocate the task stack, construct its PMP configuration, and register it.
    pub fn spawn(self) -> TaskId {
        let (stack_bottom, stack_top) = alloc_task_stack();

        match self.mode {
            TaskMode::MMode => crate::sched::scheduler::create_task_mmode(self.entry, stack_top),
            TaskMode::UMode {
                code_base,
                code_size,
                mmio,
            } => {
                let mut builder = PmpConfig::builder()
                    .code_region(code_base, code_size)
                    .stack_region(stack_bottom, crate::config::TASK_STACK_SIZE as u64);
                for &(base, size) in mmio {
                    builder = builder.mmio_region(base, size);
                }
                let pmp = builder.build();
                crate::sched::scheduler::create_task(self.entry, stack_top, &pmp.regions)
            }
        }
    }
}

fn alloc_task_stack() -> (u64, u64) {
    let layout = Layout::from_size_align(
        crate::config::TASK_STACK_SIZE,
        crate::config::TASK_STACK_ALIGN,
    )
    .expect("valid task stack layout");
    let allocation = Global.allocate(layout).expect("allocating stack").as_ptr() as *mut u8 as u64;
    (allocation, allocation + layout.size() as u64)
}
