# CyptOS Kernel Architecture

## Overview

CyptOS is a capability-based microkernel for RISC-V 64-bit, designed for security and cryptography. It follows the Sentry OS security model with build-time task metadata, capability-based access control, and PMP-based memory isolation.

### Design Goals

- **Security-first**: Capability-based access control, W^X enforcement, strict isolation
- **Minimal TCB**: Small trusted computing base, bare-metal (no SBI dependency)
- **RISC-V native**: Targets RV64GC with Privileged Specification v1.12
- **Rust-only**: No C code, leverages Rust's safety guarantees
- **Multiple images**: Separate bootloader, kernel, and task binaries

### Target Platform

| Property | Value |
|----------|-------|
| Architecture | RISC-V 64-bit (RV64GC) |
| Privilege Spec | v1.12 |
| Memory Protection | PMP (Physical Memory Protection) |
| Test Platform | QEMU virt machine |
| UART | NS16550A at 0x1000_0000 |
| Timer | CLINT at 0x200_0000 |

## Memory Layout

```
0x0000_0000 ┌─────────────────────┐
            │                     │
0x0200_0000 ├─────────────────────┤
            │ CLINT (timer)       │ 64KB
0x1000_0000 ├─────────────────────┤
            │ UART                │
            │                     │
0x8000_0000 ├─────────────────────┤
            │ RAM Start           │
0x8020_0000 ├─────────────────────┤ ← Kernel Load Address
            │ .text.entry         │ Entry point (_start)
            │ .text.trap          │ Trap vector
            │ .text               │ Kernel code
            ├─────────────────────┤
            │ .rodata             │ Read-only data
            ├─────────────────────┤
            │ .data               │ Initialized data
            ├─────────────────────┤
            │ .bss                │ Uninitialized data
            ├─────────────────────┤
            │ Kernel Stack (64KB) │
            ├─────────────────────┤
            │ Kernel Heap (16MB)  │
            ├─────────────────────┤
            │ _end                │
            │                     │
0x8800_0000 └─────────────────────┘ RAM End (128MB)
```

## Boot Sequence

```
_start (naked, .text.entry)
    │
    ├─► Set stack pointer to _stack_top
    ├─► Call clear_bss()
    └─► Call kmain()
            │
            ├─► serial::init()                         - Initialize serial backend for debug output
            ├─► ALLOCATOR.init(heap_start, heap_end)   - Initialize bump heap allocator
            ├─► trap::init()                           - Set mtvec to trap_vector
            ├─► pmp::init()                            - Configure memory protection
            ├─► timer::init()                          - Setup CLINT timer
            ├─► scheduler::create_task(user_echo_task, ...)      [PMP: code RX + stack RW + UART RW]
            ├─► scheduler::create_task(user_violation_task, ...) [PMP: code RX + stack RW, no UART]
            ├─► scheduler::init()                      - Transition Created tasks to Ready
            ├─► timer::enable_interrupts()             - Enable global interrupts
            └─► loop { wfi }      - Idle loop (scheduler runs from timer ISR)
```

## Module Reference

### serial.rs / serial/uart16550.rs - Serial Console

NS16550A UART driver for QEMU virt machine.

| Function | Description |
|----------|-------------|
| `init()` | Configure UART: 8N1, FIFO enabled |
| `putc(u8)` | Write single byte, waits for TX ready |
| `puts(&str)` | Write string, handles `\n` → `\r\n` |
| `put_dec(u64)` | Print decimal number |
| `put_hex(u64)` | Print hex number with `0x` prefix |
| `getc() -> u8` | Read single byte, blocks until available |

**Registers:**

| Offset | Name | Purpose |
|--------|------|---------|
| 0x00 | THR/RBR | Transmit Hold / Receive Buffer |
| 0x01 | IER | Interrupt Enable |
| 0x02 | FCR | FIFO Control |
| 0x03 | LCR | Line Control |
| 0x05 | LSR | Line Status |

### trap.rs - Exception & Interrupt Handling

Sets up mtvec and handles all traps in M-mode.

**Key Types:**

```rust
pub struct TrapFrame {
    // 31 general-purpose registers saved on trap entry
    pub ra: u64,
    pub t0..t6: u64,
    pub a0..a7: u64,
    pub s0..s11: u64,
    pub gp: u64,
    pub tp: u64,
    pub sp: u64,  // Original sp before trap
}

pub enum Exception { ... }  // mcause exception codes
pub enum Interrupt { ... }  // mcause interrupt codes
```

**Key Functions:**

| Function | Description |
|----------|-------------|
| `init()` | Write trap_vector address to mtvec CSR |
| `read_mcause() -> u64` | Get trap cause |
| `read_mepc() -> u64` | Get exception program counter |
| `write_mepc(u64)` | Set return address |
| `read_mtval() -> u64` | Get trap value (faulting address/instruction) |

**Trap Flow:**

```
Trap occurs
    │
    ▼
trap_vector (naked)
    ├─► Save mscratch ← sp
    ├─► Allocate TrapFrame (256 bytes)
    ├─► Save all 31 GPRs
    ├─► Call trap_handler(&mut TrapFrame)
    ├─► Restore all 31 GPRs
    └─► mret
            │
            ▼
trap_handler
    ├─► Read mcause, mepc, mtval
    ├─► Check interrupt bit (mcause[63])
    │       │
    │       ├─► Interrupt → handle_interrupt()
    │       └─► Exception → handle_exception()
    └─► Return (mret will restore context)
```

**Handled Exceptions:**

| Code | Name | Action |
|------|------|--------|
| 0-2, 12-15 | Faults | Panic with diagnostic |
| 1, 5, 7 | Access Faults | Check MPP: U-mode faults kill task + reschedule; M-mode faults panic |
| 3 | Breakpoint | Log, advance mepc by 4 |
| 8 | Ecall from U-mode | Dispatch syscall by a7: SYS_GETC (0) non-blocking UART read → a0; SYS_PUTC (1) write a0 to UART; syscall 42 legacy POC (kept for compat); advance mepc |
| 9 | Ecall from S-mode | Log, advance mepc |
| 11 | Ecall from M-mode | Log, advance mepc |

**Handled Interrupts:**

| Code | Name | Action |
|------|------|--------|
| 3 | Machine Software | Log |
| 7 | Machine Timer | Increment tick, schedule next tick, call scheduler::schedule(frame) for preemptive context switch, log every 100 ticks |
| 11 | Machine External | Log |

### pmp.rs - Physical Memory Protection

Configures RISC-V PMP for memory isolation.

**Key Types:**

```rust
pub mod flags {
    pub const R: u8 = 1 << 0;      // Read
    pub const W: u8 = 1 << 1;      // Write
    pub const X: u8 = 1 << 2;      // Execute
    pub const A_NAPOT: u8 = 3 << 3; // Naturally aligned power-of-2
    pub const L: u8 = 1 << 7;      // Lock (enforces for M-mode too)
}

pub struct PmpRegion {
    pub base: u64,
    pub size: u64,  // Must be power of 2, >= 8
    pub flags: u8,
}
```

**Key Functions:**

| Function | Description |
|----------|-------------|
| `init()` | Clear all entries, set permissive default |
| `configure_region(idx, &PmpRegion) -> bool` | Configure single NAPOT entry |
| `clear_all()` | Zero all pmpcfg and pmpaddr registers |
| `dump_config()` | Debug print pmpcfg0/pmpcfg2 |

**NAPOT Address Encoding:**

For a region with base `B` and size `S` (power of 2):
```
pmpaddr = (B + S/2 - 1) >> 2
```

**Current Configuration:**

Entry 0: Full address space (0 to 2^56) with RWX - permissive for kernel development. Will be restricted when tasks are added.

**PMP Entry Layout (RV64):**

```
pmpcfg0: [entry7][entry6][entry5][entry4][entry3][entry2][entry1][entry0]
pmpcfg2: [entry15][entry14][entry13][entry12][entry11][entry10][entry9][entry8]
         ─────────────────────────────────────────────────────────────────────
         Each entry: 8 bits = [L][0][0][A1][A0][X][W][R]
```

### timer.rs - CLINT Timer Driver

Manages machine timer for preemption.

**Constants:**

| Name | Value | Description |
|------|-------|-------------|
| CLINT_BASE | 0x200_0000 | CLINT MMIO base |
| MTIME_OFFSET | 0xBFF8 | mtime register offset |
| MTIMECMP_OFFSET | 0x4000 | mtimecmp[0] offset |
| TIMER_FREQ | 10_000_000 | 10 MHz (QEMU virt) |
| DEFAULT_TICK_US | 10_000 | 10ms tick interval |

**Key Functions:**

| Function | Description |
|----------|-------------|
| `init()` | Clear timer, enable MTIE, schedule first tick |
| `read_mtime() -> u64` | Read current timer value |
| `write_mtimecmp(u64)` | Set timer compare value |
| `enable_timer_interrupt()` | Set mie.MTIE |
| `disable_timer_interrupt()` | Clear mie.MTIE |
| `enable_interrupts()` | Set mstatus.MIE (global) |
| `disable_interrupts()` | Clear mstatus.MIE |
| `set_timer_us(u64)` | Schedule interrupt at now + microseconds |
| `clear_timer()` | Set mtimecmp to MAX (disable) |
| `schedule_next_tick()` | Schedule next 10ms tick |
| `get_tick_count() -> u64` | Get total ticks since boot (reads `AtomicCounter`) |
| `increment_tick()` | Increment `AtomicCounter` tick counter (called from ISR) |

**Timer Interrupt Flow:**

```
Timer fires (mtime >= mtimecmp)
    │
    ▼
trap_vector → trap_handler → handle_interrupt(7)
    │
    ├─► increment_tick()
    ├─► Log every 100 ticks (1 second)
    ├─► schedule_next_tick() → set_timer_us(10000)
    └─► Return from interrupt
```

### allocator.rs - Bump Heap Allocator

Provides a simple bump allocator registered as the global Rust allocator.

| Item | Description |
|------|-------------|
| `BumpAllocator` | Struct implementing `GlobalAlloc` |
| `new()` | Const constructor, uninitialized state |
| `init(heap_start, heap_end)` | Initialize from linker symbols; sets base and end pointers |
| `alloc()` | Bump pointer forward with alignment, returns null on OOM |
| `dealloc()` | No-op (non-freeing allocator) |

**Notes:** Uses `AtomicUsize` for the bump pointer. Single-hart only; no concurrent alloc support.

---

### sched/task.rs - Task Metadata

Defines task structures and state machine used by the scheduler.

| Item | Description |
|------|-------------|
| `MAX_TASKS` | `4` - maximum simultaneous tasks |
| `TaskContext` | 31 GPRs + mepc + mstatus (264 bytes, `repr(C)`) |
| `TaskContext::new_umode(entry, stack_top)` | Initialize context for U-mode: MPP=00, MPIE=1 |
| `TaskState` | Enum: `Created`, `Ready`, `Running`, `Dead` |
| `TaskId(u8)` | Newtype wrapper for task index |
| `PmpConfig` | 16 `PmpRegion` entries per task |
| `Task` | `id`, `state`, `context`, `pmp_config`, `stack_bottom`, `stack_top`, `entry_point` |

---

### sched/scheduler.rs - Round-Robin Scheduler

Manages task lifecycle and performs preemptive context switches.

| Item | Description |
|------|-------------|
| `SCHEDULER` | `IrqCell<SchedulerState>` — interrupt-disable protected state |
| `SchedulerState.current` | `usize`; `usize::MAX` means idle |
| `SchedulerState.tasks` | `[Option<Task>; MAX_TASKS]` task table |
| `init()` | Transition all `Created` tasks to `Ready` |
| `schedule(&mut TrapFrame)` | Save current context, round-robin pick next `Ready` task, swap PMP, restore context + CSRs |
| `create_task(entry, stack_top, &[PmpRegion])` | Allocate task slot, return `TaskId` |
| `kill_current(&mut TrapFrame)` | Mark current task `Dead`, call `schedule()` |

**Context Switch Flow:**

```
Timer ISR fires
    │
    ▼
schedule(&mut TrapFrame)
    ├─► Save GPRs + mepc + mstatus from TrapFrame into current TaskContext
    ├─► Find next Ready task (round-robin)
    ├─► clear_all() + configure PMP regions for next task
    ├─► Restore next task's GPRs + mepc + mstatus into TrapFrame
    └─► mret → next task resumes
```

---

### user_task.rs - U-mode Tasks

User-space task code running in U-mode (MPP=00), placed in `.user_text` section.

| Item | Description |
|------|-------------|
| `SYS_GETC` | Syscall number `0` - non-blocking UART read |
| `SYS_PUTC` | Syscall number `1` - write byte to UART |
| `user_echo_task()` | Polls `SYS_GETC`, echoes received bytes via `SYS_PUTC`; maps `\r` → `\n` |
| `user_violation_task()` | Reads address `0x8000_0000` to trigger Load Access Fault (intentional isolation demo) |

**Notes:** Both tasks run in U-mode with per-task PMP configurations. A fault in `user_violation_task` is caught by the trap handler, the task is killed, and the scheduler reschedules.

---

## Synchronization Model

CyptOS is currently single-hart, so synchronization needs are modest.

| Primitive | Type | Usage |
|-----------|------|-------|
| `IrqCell<T>` | Interrupt-disable wrapper | Protects shared kernel state (scheduler task table, current task index) |
| `AtomicCounter` | Lock-free atomic `u64` | Tick counter incremented from the timer ISR |

`IrqCell<T>` disables machine interrupts (`mstatus.MIE`) for the duration of a critical section, matching the Linux pattern for single-CPU interrupt-disable locks. `AtomicCounter` uses `core::sync::atomic::AtomicU64` and requires no interrupt masking.

When multi-hart support is added, `IrqCell` will be replaced or supplemented with spinlocks (ticket or MCS) and per-hart state will be introduced.

## CSR Access Layer

All CSR reads and writes outside naked assembly functions go through `arch::csr`. The module provides:

- Macro-based `csr_read!` / `csr_write!` primitives wrapping inline asm
- Typed wrappers for `mstatus`, `mcause`, `mepc`, `mtval`, `mtvec`, and `mie`

This keeps raw CSR names out of the trap, timer, and scheduler modules and makes the access points easy to audit.

---

## Security Model (Planned)

Based on Sentry OS capability model:

### Capabilities

Access rights defined at build time per task:

| Capability | Description |
|------------|-------------|
| DEV_* | Device access (UART, SPI, I2C, etc.) |
| MEM_* | Shared memory regions |
| IRQ_* | Interrupt handling rights |
| DMA_* | DMA channel access |
| SYS_* | Syscall classes (IPC, SLEEP, etc.) |

### Isolation Mechanisms

1. **PMP Regions**: Per-task memory boundaries enforced in hardware
2. **Capability Checks**: Every syscall validates caller's capability set
3. **W^X**: Memory is either writable or executable, never both
4. **Build-time Metadata**: Task capabilities frozen at compile time

## File Structure

```
cyptos/
├── Cargo.toml
├── Makefile
├── .cargo/
│   └── config.toml
├── board/
│   └── qemu-virt/
│       └── linker.ld
├── crates/
│   └── kernel/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs          # Entry point, kmain, panic handler
│           ├── config.rs        # Centralized constants (MMIO, syscalls, limits)
│           ├── arch.rs          # RISC-V architecture abstractions
│           ├── arch/
│           │   ├── csr.rs       # CSR read/write macros + typed wrappers
│           │   └── register.rs  # GPR copy macro
│           ├── sync.rs          # Synchronization primitives
│           ├── sync/
│           │   ├── irq_cell.rs  # Interrupt-disable critical section
│           │   └── atomic_counter.rs
│           ├── sched.rs         # Scheduling subsystem
│           ├── sched/
│           │   ├── task.rs      # Task types, PmpConfig builder
│           │   └── scheduler.rs # Round-robin scheduler
│           ├── trap.rs          # Trap vector and handlers
│           ├── pmp.rs           # PMP configuration
│           ├── timer.rs         # CLINT timer driver
│           ├── allocator.rs     # Bump heap allocator
│           ├── serial.rs        # Serial backend selection
│           ├── serial/
│           │   ├── uart16550.rs # NS16550A UART driver
│           │   └── usart_stm32.rs # STM32 stub
│           └── user_task.rs     # U-mode task code
├── crates/
│   ├── cyptos-test/
│   └── cyptos-test-macros/
├── docs/
│   ├── architecture.md
│   └── testing.md
└── tools/docker/Dockerfile
```

## Build & Run

```bash
# Build kernel
cd cyptos
cargo build --release

# Output binary
target/riscv64gc-unknown-none-elf/release/kernel

# Run in QEMU (requires Docker setup or local QEMU)
make docker-build && make run

# Or directly with QEMU
qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -m 128M \
    -nographic \
    -bios none \
    -kernel target/riscv64gc-unknown-none-elf/release/kernel
```

## Development Roadmap

### Phase 0 - Bootstrap ✓
- [x] Bare-metal entry point
- [x] BSS initialization
- [x] UART output
- [x] Panic handler

### Phase 1 - Memory & Traps ✓
- [x] Trap vector setup (mtvec)
- [x] Exception/interrupt handling
- [x] PMP driver
- [x] CLINT timer

### Phase 2 - Task Model ✓
- [x] Task metadata struct
- [x] Static task table
- [x] Task state machine
- [x] Context switch

### Phase 2.5 - Heap & User Tasks ✓
- [x] Bump heap allocator
- [x] U-mode task execution
- [x] Syscall interface (SYS_GETC, SYS_PUTC)
- [x] Graceful fault handling (kill + reschedule)

### Phase 3 - Capability System
- [ ] Capability types
- [ ] Per-task capability bitfield
- [ ] Syscall capability checks

### Phase 4 - Scheduler (Partial) ✓
- [x] Round-robin scheduler
- [ ] Priority queues
- [ ] Idle task
- [x] PMP swap on context switch

### Phase 5 - Syscalls & IPC
- [ ] Syscall dispatch
- [ ] IPC primitives
- [ ] Shared memory

### Phase 6+ - Drivers & Hardening
- [ ] Additional device drivers
- [ ] Security hardening
- [ ] Cryptographic services
