//! Task scheduling subsystem for CyptOS.
//!
//! Contains task metadata, the round-robin scheduler that runs from the timer
//! ISR, and [`TaskSpec`] for registering tasks at boot. Scheduler state is
//! protected by `IrqCell` — see `sync` module.

mod policy;
mod scheduler;
mod task;

pub use scheduler::{init, kill_current, schedule};
pub use task::{TaskContext, TaskSpec};
