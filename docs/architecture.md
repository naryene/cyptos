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
            ├─► serial::init()    - Initialize serial backend for debug output
            ├─► trap::init()      - Set mtvec to trap_vector
            ├─► pmp::init()       - Configure memory protection
            ├─► timer::init()     - Setup CLINT timer
            ├─► timer::enable_interrupts() - Enable global interrupts
            └─► loop { wfi }      - Idle loop (wait for interrupt)
```

## Module Reference

### serial/uart16550.rs - Serial Console

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
| 0-2, 4-7, 12-15 | Faults | Panic with diagnostic |
| 3 | Breakpoint | Log, advance mepc by 4 |
| 8 | Ecall from U-mode | Log (syscall placeholder), advance mepc |
| 9 | Ecall from S-mode | Log, advance mepc |
| 11 | Ecall from M-mode | Log, advance mepc |

**Handled Interrupts:**

| Code | Name | Action |
|------|------|--------|
| 3 | Machine Software | Log |
| 7 | Machine Timer | Increment tick, schedule next, log every 100 ticks |
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
| `get_tick_count() -> u64` | Get total ticks since boot |
| `increment_tick()` | Increment tick counter (called from ISR) |

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
├── Cargo.toml              # Workspace config
├── .cargo/
│   └── config.toml         # Target: riscv64gc-unknown-none-elf
├── board/
│   └── qemu-virt/
│       └── linker.ld       # Memory layout for QEMU virt
├── crates/
│   └── kernel/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs     # Entry point, kmain, panic handler
│           ├── serial/     # Serial backends (feature-selected)
│           │   ├── mod.rs
│           │   ├── uart16550.rs
│           │   └── usart_stm32.rs (stub)
│           ├── trap.rs     # Trap vector and handlers
│           ├── pmp.rs      # PMP configuration
│           └── timer.rs    # CLINT timer driver
└── docs/
    └── architecture.md     # This document
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

### Phase 2 - Task Model (Next)
- [ ] Task metadata struct
- [ ] Build-time task table
- [ ] Task state machine
- [ ] Context switch

### Phase 3 - Capability System
- [ ] Capability types
- [ ] Per-task capability bitfield
- [ ] Syscall capability checks

### Phase 4 - Scheduler
- [ ] Round-robin scheduler
- [ ] Priority queues
- [ ] Idle task
- [ ] PMP swap on context switch

### Phase 5 - Syscalls & IPC
- [ ] Syscall dispatch
- [ ] IPC primitives
- [ ] Shared memory

### Phase 6+ - Drivers & Hardening
- [ ] Additional device drivers
- [ ] Security hardening
- [ ] Cryptographic services
