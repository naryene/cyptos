//! Task scheduling subsystem for CyptOS.
//!
//! Contains the task control block (`Task`, `TaskContext`, `PmpConfig`), the
//! round-robin scheduler that runs from the timer ISR, and the `create_task`
//! API used by `kmain` to register user-mode tasks at boot. Scheduler state
//! is protected by `IrqCell` — see `sync` module.

mod scheduler;
mod task;

pub use scheduler::{create_task, create_task_mmode, init, kill_current, schedule};
pub use task::{PmpConfig, TaskContext};
