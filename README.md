# CyptOS

A security-focused RISC-V 64-bit microkernel written in Rust.

## Features

- Bare-metal boot on RISC-V 64-bit (hart 0; secondary harts parked via WFI)
- NS16550A UART serial driver with build-time backend selection
- M-mode trap handling via `mtvec` direct mode (exceptions and interrupts)
- Physical Memory Protection (PMP) with NAPOT encoding and per-task regions
- CLINT timer driver with 10ms preemption tick
- Bump heap allocator bounded by linker symbols
- Round-robin preemptive scheduler with full CPU context save/restore (31 GPRs + mepc + mstatus)
- U-mode task execution with PMP isolation
- Syscall interface: `SYS_GETC` (0), `SYS_PUTC` (1)
- Graceful U-mode fault handling: kill faulting task and reschedule
- Custom test framework (`cyptos-test`) supporting both host and bare-metal targets

## Prerequisites

Docker. Everything else (Rust nightly, QEMU, binutils) lives inside the container.

```sh
make docker-build
```

## Quick Start

Build the Docker development image (first time only), then run the kernel in QEMU:

```sh
make docker-build
make run
```

For a debug session with GDB attached on port 1234:

```sh
make debug
# In a second terminal:
gdb-multiarch -ex 'target remote :1234' target/riscv64gc-unknown-none-elf/release/kernel
```

## Build Commands

| Command             | Description                                       |
|---------------------|---------------------------------------------------|
| `make build`        | Debug build                                       |
| `make release`      | Release build + strip to flat binary              |
| `make run`          | Build (release) and run in QEMU                   |
| `make debug`        | Run in QEMU with GDB server on port 1234          |
| `make test`         | Run host unit tests (`cyptos-test` crate)         |
| `make check`        | Type-check without building                       |
| `make fmt`          | Format all crates                                 |
| `make clippy`       | Lint all targets                                  |
| `make doc`          | Generate rustdoc                                  |
| `make audit`        | Audit unsafe blocks via clippy lints              |
| `make objdump`      | Disassemble kernel ELF                            |
| `make size`         | Show kernel binary size                           |
| `make clean`        | Remove build artifacts                            |
| `make docker-build` | Build the Docker development image                |
| `make docker-shell` | Open an interactive shell in the container        |

All build commands run inside Docker. Do not invoke `cargo` directly.

## Project Structure

```
cyptos/
├── Cargo.toml                  # Workspace (edition 2024, no_std)
├── Makefile                    # Docker-based build system
├── .cargo/config.toml          # riscv64gc-unknown-none-elf target
├── board/qemu-virt/linker.ld   # Linker script for QEMU virt machine
├── crates/
│   ├── kernel/src/
│   │   ├── main.rs             # Entry point, kmain, panic handler
│   │   ├── config.rs           # Centralized constants
│   │   ├── arch/               # CSR access, register macros
│   │   ├── sync/               # IrqCell, AtomicCounter
│   │   ├── sched/              # Task types, round-robin scheduler
│   │   ├── trap.rs             # Trap vector and handlers
│   │   ├── pmp.rs              # PMP configuration
│   │   ├── timer.rs            # CLINT timer driver
│   │   ├── allocator.rs        # Bump heap allocator
│   │   ├── serial/             # UART backends (NS16550A, STM32 stub)
│   │   └── user_task.rs        # U-mode task implementations
│   ├── cyptos-test/            # Test runtime (host + bare-metal)
│   └── cyptos-test-macros/     # #[cyptos_test] procedural macro
├── docs/
│   ├── architecture.md
│   └── testing.md
└── tools/docker/Dockerfile
```

## Architecture

CyptOS runs entirely in M-mode with user tasks demoted to U-mode. Memory isolation is enforced through PMP regions configured per task before each context switch. The scheduler is interrupt-driven: the CLINT timer fires every 10ms, triggers an M-mode trap, and the trap handler invokes the scheduler to select the next runnable task. Kernel shared state is protected by interrupt-disable critical sections (IrqCell), and all CSR access is routed through typed wrappers in the arch module.

See [docs/architecture.md](docs/architecture.md) for a detailed breakdown.

## QEMU Configuration

| Parameter | Value                      |
|-----------|----------------------------|
| Machine   | `virt`                     |
| SMP       | 4 cores                    |
| RAM       | 128 MB                     |
| BIOS      | none (bare-metal)          |
| UART      | NS16550A at `0x1000_0000`  |
| Timer     | CLINT at 10 MHz, 10ms tick |

## Roadmap

- [x] Bare-metal boot and UART output
- [x] M-mode trap handling
- [x] PMP-based U-mode isolation
- [x] Preemptive round-robin scheduler
- [x] Syscall interface
- [x] U-mode fault recovery
- [ ] Virtual memory (Sv39 page tables)
- [ ] Inter-process communication
- [ ] Capability-based access control
- [ ] Persistent storage driver
- [ ] Real hardware target (SiFive HiFive)

## License

Licensed under either of [MIT](LICENSE-MIT)
