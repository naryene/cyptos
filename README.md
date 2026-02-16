# CyptOS

A capability-based microkernel for RISC-V 64-bit, focused on security and cryptography.

## Overview

CyptOS is a bare-metal microkernel targeting RV64GC, written entirely in Rust with no C code. It runs in M-mode without SBI/OpenSBI, using PMP (Physical Memory Protection) for hardware-enforced memory isolation. The security model is inspired by [Sentry OS](https://github.com/outpost-os/sentry-kernel), with build-time task metadata and capability-based access control. Targets the RISC-V Privileged Specification v1.12.

Currently in early development.

## Features

- Bare-metal M-mode execution (no SBI/OpenSBI dependency)
- SMP-aware boot (hart 0 boots, others park via WFI)
- NS16550A UART driver with build-time serial backend selection (UART/USART)
- Full trap handling (exceptions + interrupts) with 31-register TrapFrame save/restore
- PMP driver with NAPOT encoding (16 entries)
- CLINT timer with 10ms periodic tick
- Custom test framework (`#[cyptos_test]`) working on both host and bare-metal

## Prerequisites

Docker. Everything else (Rust nightly, QEMU, binutils) lives inside the container.

```bash
# Build the development container (first time only)
make docker-build
```

## Quick Start

```bash
# Build and run in QEMU
make run

# Build only (debug)
make build

# Build only (release)
make release
```

## Make Targets

| Target | Description |
|--------|-------------|
| `make run` | Build release and run in QEMU |
| `make build` | Debug build |
| `make release` | Release build + objcopy to raw binary |
| `make test` | Run host tests (`x86_64`) |
| `make debug` | Run with GDB server on port 1234 |
| `make check` | Type-check without building |
| `make fmt` | Format code |
| `make clippy` | Lint |
| `make objdump` | Disassemble kernel ELF |
| `make size` | Show kernel binary size |
| `make docker-shell` | Open shell in the build container |
| `make clean` | Remove build artifacts |

## Project Structure

```
cyptos/
├── board/
│   └── qemu-virt/
│       └── linker.ld              # Memory layout
├── crates/
│   ├── kernel/                    # The microkernel
│   │   └── src/
│   │       ├── main.rs            # Entry point, boot, panic handler
│   │       ├── trap.rs            # Exception/interrupt handling
│   │       ├── pmp.rs             # Physical Memory Protection
│   │       ├── timer.rs           # CLINT timer driver
│   │       └── serial/            # Serial backends (feature-gated)
│   ├── cyptos-test/               # Test runtime (host + bare-metal)
│   └── cyptos-test-macros/        # #[cyptos_test] proc macro
├── docs/
│   ├── architecture.md            # Kernel architecture reference
│   └── testing.md                 # Test framework usage
├── tools/
│   └── docker/
│       └── Dockerfile             # Development environment
├── Cargo.toml                     # Workspace config
├── Makefile                       # Build system
└── rust-toolchain.toml            # Pinned to nightly-2026-02-14
```

## Documentation

- [Kernel Architecture](docs/architecture.md) — memory layout, boot sequence, module reference, security model
- [Testing](docs/testing.md) — how to use `#[cyptos_test]` on host and bare-metal

## Roadmap

- ✅ Phase 0: Bootstrap (entry point, BSS init, UART, panic handler)
- ✅ Phase 1: Memory & Traps (trap vector, PMP driver, CLINT timer)
- 🔲 Phase 2: Task Model (metadata struct, task table, state machine, context switch)
- 🔲 Phase 3: Capability System (types, per-task bitfield, syscall checks)
- 🔲 Phase 4: Scheduler (round-robin, priority queues, PMP swap on context switch)
- 🔲 Phase 5: Syscalls & IPC
- 🔲 Phase 6: Drivers & Hardening

## License

MIT OR Apache-2.0
