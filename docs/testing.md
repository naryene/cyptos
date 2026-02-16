# CyptOS Testing

CyptOS uses `cyptos-test` and `cyptos-test-macros` to provide a unified
`#[cyptos_test]` attribute that works as a drop-in `#[test]` replacement on
both **host** (`x86_64`, std) and **bare-metal riscv64** (QEMU, no_std).

## Host tests

`#[cyptos_test]` expands to `#[test]`. No special configuration needed.

```rust
use cyptos_test::cyptos_test;

#[cyptos_test]
fn arithmetic_works() {
    assert_eq!(2 + 2, 4);
}
```

```bash
cargo test --target x86_64-unknown-linux-gnu -p your-crate
```

## Kernel / bare-metal tests (riscv64)

`#[cyptos_test]` expands to `#[test_case]`, used with Rust's
`custom_test_frameworks` feature. Add this to your crate root:

```rust
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(cyptos_test::runner)]
#![reexport_test_harness_name = "test_main"]

use cyptos_test::cyptos_test;

#[cyptos_test]
fn pmp_basic() {
    // test logic
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    cyptos_test::test_panic_handler(info)
}
```

The runner outputs results over UART and exits QEMU via the sifive_test
device (`0x10_0000`).

## Build commands

```bash
# Host tests (no build-std needed)
cargo test --target x86_64-unknown-linux-gnu -p your-crate

# Kernel build (requires build-std)
cargo build -Z build-std=core,alloc,compiler_builtins \
            -Z build-std-features=compiler-builtins-mem

# Or use make
make test     # host tests
make build    # kernel build (includes build-std)
```

## Architecture

- `cyptos-test-macros` — proc macro crate; emits `#[test]` (host) or
  `#[test_case]` (riscv64) via `cfg_attr(target_arch)`.
- `cyptos-test` — runtime crate; `Testable` trait, `runner()`, platform
  output (UART on riscv64, stderr on host), QEMU exit helpers.
