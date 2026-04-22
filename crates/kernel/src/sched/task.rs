//! Task metadata and state machine for CyptOS.
//!
//! Defines the task lifecycle types used by the scheduler and context switch.
//! Reference: RISC-V Privileged Specification v1.12, Section 3.1.6 (mstatus layout)

pub use crate::config::MAX_TASKS;
use crate::config::PMP_COUNT;
use crate::pmp::PmpRegion;

/// Saved CPU context for a task.
///
/// Layout is a superset of TrapFrame — the first 31 fields (GPRs) maintain
/// identical offsets so assembly save/restore code works for both.
/// mepc and mstatus extend the frame for full task context preservation.
///
/// Field order and offsets (MUST NOT change — assembly depends on these):
/// - ra:      offset 0
/// - t0..t6: offsets 8..56
/// - a0..a7: offsets 64..120
/// - s0..s11: offsets 128..216
/// - gp:     offset 224
/// - tp:     offset 232
/// - sp:     offset 240
/// - mepc:   offset 248
/// - mstatus: offset 256
///   Total: 33 * 8 = 264 bytes → align to 16 → 272 bytes
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    // Caller-saved registers
    pub ra: u64,
    pub t0: u64,
    pub t1: u64,
    pub t2: u64,
    pub t3: u64,
    pub t4: u64,
    pub t5: u64,
    pub t6: u64,
    // Function argument / return value registers
    pub a0: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
    pub a7: u64,
    // Callee-saved registers
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
    // Platform registers
    pub gp: u64,
    pub tp: u64,
    pub sp: u64,
    // Machine CSRs (not in TrapFrame — context switch saves/restores these)
    pub mepc: u64,
    pub mstatus: u64,
}

impl TaskContext {
    /// Create a zeroed task context suitable for a new M-mode task (e.g. idle task).
    ///
    /// Sets mepc to `entry_point` and mstatus to MPP=11 (M-mode) with MPIE=1.
    pub const fn new_mmode(entry_point: u64, stack_top: u64) -> Self {
        // mstatus: MPP=11 (M-mode, bits 12:11), MPIE=1 (bit 7)
        let mstatus = (0b11u64 << 11) | (1u64 << 7);
        Self {
            ra: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            gp: 0,
            tp: 0,
            sp: stack_top,
            mepc: entry_point,
            mstatus,
        }
    }

    /// Create a zeroed task context suitable for a new U-mode task.
    ///
    /// Sets mepc to `entry_point` and mstatus to MPP=00 (U-mode) with MPIE=1.
    pub const fn new_umode(entry_point: u64, stack_top: u64) -> Self {
        // mstatus: MPP=00 (U-mode, bits 12:11 cleared), MPIE=1 (bit 7 set)
        // All other bits 0 (FPU disabled: FS=00, no extensions)
        let mstatus = 0b0000_0001u64 << 7; // MPIE=1, MPP=00
        Self {
            ra: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            gp: 0,
            tp: 0,
            sp: stack_top,
            mepc: entry_point,
            mstatus,
        }
    }
}

/// Task execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    /// Task has been created but not yet scheduled.
    Created = 0,
    /// Task is ready to run (not blocked, not currently running).
    Ready = 1,
    /// Task is currently executing on the CPU.
    Running = 2,
    /// Task has terminated and its slot may be reclaimed.
    Dead = 3,
}

/// Opaque task identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub u8);

/// Per-task PMP configuration.
///
/// Stores 16 PMP region descriptors that are loaded when this task
/// is scheduled (via `pmp::load_task_config`).
#[derive(Debug, Clone, Copy)]
pub struct PmpConfig {
    pub regions: [PmpRegion; PMP_COUNT],
}

impl PmpConfig {
    /// Creates a zeroed PMP configuration (all regions disabled).
    pub const fn empty() -> Self {
        Self {
            regions: [PmpRegion::new(0, 0, 0); PMP_COUNT],
        }
    }

    /// Create a builder for ergonomic PMP region setup.
    ///
    /// User-task entries start at index 4; entries 0-3 are reserved for the
    /// locked kernel regions installed by `pmp::init()`.
    pub fn builder() -> PmpConfigBuilder {
        PmpConfigBuilder {
            regions: [PmpRegion::new(0, 0, 0); PMP_COUNT],
            next_idx: 4,
        }
    }
}

/// Builder for [`PmpConfig`].
///
/// Entries 0-3 are reserved for locked kernel PMP regions and must not be
/// written here. The builder starts at index 4 and advances on each call.
pub struct PmpConfigBuilder {
    regions: [PmpRegion; PMP_COUNT],
    next_idx: usize,
}

impl PmpConfigBuilder {
    /// Add a code region (Read + Execute).
    pub fn code_region(mut self, base: u64, size: u64) -> Self {
        assert!(self.next_idx < PMP_COUNT, "PMP region table full");
        assert!(size.is_power_of_two(), "PMP region size must be power of 2");
        assert!(
            base & (size - 1) == 0,
            "PMP region base must be aligned to size"
        );
        self.regions[self.next_idx] = PmpRegion::new(base, size, crate::pmp::flags::RX);
        self.next_idx += 1;
        self
    }

    /// Add a stack region (Read + Write).
    pub fn stack_region(mut self, base: u64, size: u64) -> Self {
        assert!(self.next_idx < PMP_COUNT, "PMP region table full");
        assert!(size.is_power_of_two(), "PMP region size must be power of 2");
        assert!(
            base & (size - 1) == 0,
            "PMP region base must be aligned to size"
        );
        self.regions[self.next_idx] = PmpRegion::new(base, size, crate::pmp::flags::RW);
        self.next_idx += 1;
        self
    }

    /// Add an MMIO region (Read + Write).
    pub fn mmio_region(mut self, base: u64, size: u64) -> Self {
        assert!(self.next_idx < PMP_COUNT, "PMP region table full");
        assert!(size.is_power_of_two(), "PMP region size must be power of 2");
        assert!(
            base & (size - 1) == 0,
            "PMP region base must be aligned to size"
        );
        self.regions[self.next_idx] = PmpRegion::new(base, size, crate::pmp::flags::RW);
        self.next_idx += 1;
        self
    }

    /// Finalize the PMP configuration.
    pub fn build(self) -> PmpConfig {
        PmpConfig {
            regions: self.regions,
        }
    }
}

/// Task control block.
///
/// Contains everything the scheduler needs to manage a task:
/// its CPU context, PMP configuration, and stack bounds.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// Current execution state.
    pub state: TaskState,
    /// Saved CPU context (registers + CSRs).
    pub context: TaskContext,
    /// PMP configuration for this task's address space.
    pub pmp_config: PmpConfig,
    /// Bottom of task stack (lowest valid address).
    pub stack_bottom: u64,
    /// Top of task stack (initial stack pointer).
    pub stack_top: u64,
    /// Entry point address (initial mepc).
    pub entry_point: u64,
}

impl Task {
    /// Create a new task in the Created state.
    ///
    /// The task is ready to be added to the scheduler table. It will transition
    /// to Ready when `scheduler::init()` prepares it for scheduling.
    pub fn new(id: TaskId, entry: u64, stack_top: u64) -> Self {
        Self {
            id,
            state: TaskState::Created,
            context: TaskContext::new_umode(entry, stack_top),
            pmp_config: PmpConfig::empty(),
            stack_bottom: 0, // Set by caller after stack allocation
            stack_top,
            entry_point: entry,
        }
    }
}
