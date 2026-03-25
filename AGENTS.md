# CyptOS Agent Guide

This file orients agentic coders working in this repository. Follow the
commands and conventions below to keep changes consistent and buildable.

## Repo overview
- Project: CyptOS (RISC-V 64-bit capability-based microkernel in Rust).
- Workspace: `crates/kernel`, `crates/cyptos-test`, `crates/cyptos-test-macros`.
- Kernel is `no_std`/`no_main` and uses MMIO and inline assembly.
- Toolchain pinned in `rust-toolchain.toml` (nightly-2026-02-14).

## Environment & prerequisites
- Docker is required; build/test commands use a containerized toolchain.
- Preferred workflow: `make docker-build` once, then use `make` targets.
- Rust nightly components: `rust-src`, `llvm-tools-preview`, `rustfmt`, `clippy`.

## Build, run, lint, test (Makefile)
All Make targets run inside Docker and assume repo root as the workspace.

### Core targets
- `make build`        → debug build (uses build-std flags).
- `make release`      → release build + objcopy to raw kernel binary.
- `make run`          → build release and run in QEMU.
- `make debug`        → run QEMU with GDB server enabled (port 1234).

### Quality targets
- `make test`         → host tests (x86_64) for test crate.
- `make fmt`          → format all Rust code.
- `make fmt-check`    → format check only (CI-friendly).
- `make check`        → type-check with build-std flags.
- `make clippy`       → lint (warnings are errors).
- `make audit`        → stricter clippy for unsafe-block hygiene.
- `make doc`          → generate docs (private items included).

### Utilities
- `make objdump`      → disassemble kernel ELF.
- `make size`         → show kernel size.
- `make docker-shell` → interactive shell in build container.
- `make clean`        → remove build artifacts.

## Cargo equivalents (inside container)
`Makefile` targets map to the following cargo commands:

- Kernel build (build-std):
  - `cargo build -Z build-std=core,alloc,compiler_builtins \
          -Z build-std-features=compiler-builtins-mem`
- Release build:
  - `cargo build --release -Z build-std=core,alloc,compiler_builtins \
          -Z build-std-features=compiler-builtins-mem`
- Host tests (x86_64):
  - `cargo test --target x86_64-unknown-linux-gnu -p <crate>`

## Running a single test (host)
Use cargo’s standard test filter with the host target:

```bash
cargo test --target x86_64-unknown-linux-gnu -p <crate> <test_name>
```

Notes:
- `<crate>` is one of `cyptos-test`, `cyptos-test-macros`, or another workspace crate.
- Cargo’s filter matches by substring; use the exact test name to narrow.

## Testing framework notes
- Use `#[cyptos_test]` for tests that must work on both host and bare-metal.
- Bare-metal tests require `#![feature(custom_test_frameworks)]` and
  `#![test_runner(cyptos_test::runner)]` in the crate root.
- See `docs/testing.md` for full examples and QEMU exit details.

## Code style & conventions
Follow existing patterns in `crates/kernel/src/*` and test crates.

### Formatting
- Use `rustfmt` (no repo-level rustfmt config; default formatting applies).
- Keep lines readable; use trailing commas in multiline items.

### Imports
- Prefer explicit import lists (e.g., `use core::arch::{asm, naked_asm};`).
- In kernel (`no_std`), use `core`/`alloc` instead of `std`.
- Avoid unused imports; allow only when a cfg-gated re-export needs it.

### Naming
- Types: `CamelCase` (`TrapFrame`, `PmpRegion`).
- Functions/vars: `snake_case` (`read_mcause`, `set_timer_us`).
- Constants: `SCREAMING_SNAKE_CASE` (`CLINT_BASE`, `DEFAULT_TICK_US`).
- Modules: lowercase (`trap`, `timer`, `serial`).

### Unsafe & low-level code
- Keep `unsafe` blocks small and localized.
- Use `read_volatile`/`write_volatile` for MMIO.
- Prefer `#[unsafe(...)]` attributes where required (`naked`, `no_mangle`).
- Use `debug_assert!` for invariants that protect unsafe logic (see `pmp.rs`).
- Avoid `unwrap`/`expect` in kernel code; log via `serial::puts` when needed.

### Error handling
- `panic = "abort"` is configured; do not rely on unwinding.
- Kernel code should fail fast and print minimal diagnostics to UART.

### Feature flags & cfg
- Serial backends are feature-gated (`serial-uart16550` default).
- Use `cfg`/`cfg_attr` for target-specific behavior.
- If features conflict, prefer `compile_error!` like `serial/mod.rs`.

### Tests
- For host tests, `#[cyptos_test]` expands to `#[test]`.
- For bare-metal tests, it expands to `#[test_case]` with custom harness.

## Known clippy caveat
`make clippy` uses `--all-targets` which tries to build test harnesses for the
bare-metal kernel crate; this fails with `can't find crate for test` on riscv64.
This is expected. To lint only the main binary target, run clippy without
`--all-targets`:

```bash
cargo clippy -Z build-std=core,alloc,compiler_builtins \
  -Z build-std-features=compiler-builtins-mem -- -D warnings
```

## Workspace profiles
Both `dev` and `release` profiles set `panic = "abort"`. Release additionally
enables LTO and `opt-level = "z"` for size.

## Linker & memory
- Linker script: `board/qemu-virt/linker.ld`.
- Kernel loads at `0x8020_0000`, RAM `0x8000_0000`–`0x8800_0000` (128 MB).
- Stack: 64 KB, heap: 16 MB.
- Rustflags in `.cargo/config.toml`: `-C link-arg=-Tboard/qemu-virt/linker.ld`
  and `-C force-frame-pointers=yes`.

## Configuration references
- `.cargo/config.toml` sets default target to `riscv64gc-unknown-none-elf` and
  configures QEMU as the runner.
- `rust-toolchain.toml` pins nightly and required components.

## Cursor/Copilot rules
No Cursor rules found in `.cursor/rules/` or `.cursorrules`.
No Copilot instructions found in `.github/copilot-instructions.md`.

## Documentation
- `README.md` — project overview and Make targets.
- `docs/testing.md` — test framework usage and commands.
- `docs/architecture.md` — kernel architecture and memory map.
