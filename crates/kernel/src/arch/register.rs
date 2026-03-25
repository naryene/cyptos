//! Macros for bulk copying of RISC-V general-purpose registers.
//!
//! `copy_gpr_fields!` transfers all 31 GPRs (ra, t0–t6, a0–a7, s0–s11, gp, tp, sp)
//! between any two structs with matching field names — used by the scheduler to
//! move register state between `TrapFrame` (on the kernel stack) and `TaskContext`
//! (in the task control block) during context switches.

/// Copy all 31 general-purpose register fields from `$src` to `$dst`.
/// Works with any two structs that have matching GPR field names
/// (TrapFrame, TaskContext).
macro_rules! copy_gpr_fields {
    ($src:expr, $dst:expr) => {
        $dst.ra = $src.ra;
        $dst.t0 = $src.t0;
        $dst.t1 = $src.t1;
        $dst.t2 = $src.t2;
        $dst.t3 = $src.t3;
        $dst.t4 = $src.t4;
        $dst.t5 = $src.t5;
        $dst.t6 = $src.t6;
        $dst.a0 = $src.a0;
        $dst.a1 = $src.a1;
        $dst.a2 = $src.a2;
        $dst.a3 = $src.a3;
        $dst.a4 = $src.a4;
        $dst.a5 = $src.a5;
        $dst.a6 = $src.a6;
        $dst.a7 = $src.a7;
        $dst.s0 = $src.s0;
        $dst.s1 = $src.s1;
        $dst.s2 = $src.s2;
        $dst.s3 = $src.s3;
        $dst.s4 = $src.s4;
        $dst.s5 = $src.s5;
        $dst.s6 = $src.s6;
        $dst.s7 = $src.s7;
        $dst.s8 = $src.s8;
        $dst.s9 = $src.s9;
        $dst.s10 = $src.s10;
        $dst.s11 = $src.s11;
        $dst.gp = $src.gp;
        $dst.tp = $src.tp;
        $dst.sp = $src.sp;
    };
}

pub(crate) use copy_gpr_fields;
