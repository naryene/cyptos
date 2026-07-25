//! Serial I/O abstraction with compile-time backend selection.
//!
//! The active UART driver is chosen via Cargo features: `serial-uart16550` for
//! the NS16550A on QEMU virt, or `serial-usart-stm32` for STM32 hardware (stub).
//! All kernel code calls `serial::puts()`, `serial::putc()`, etc. without knowing
//! which backend is active.
#![allow(unused_imports)]

use core::fmt;

#[cfg(feature = "serial-uart16550")]
pub mod uart16550;

#[cfg(feature = "serial-usart-stm32")]
pub mod usart_stm32;

#[cfg(feature = "serial-uart16550")]
pub use uart16550::{getc, init, put_dec, put_hex, putc, puts, try_getc};

#[cfg(feature = "serial-usart-stm32")]
pub use usart_stm32::{getc, init, put_dec, put_hex, putc, puts};

#[cfg(all(feature = "serial-uart16550", feature = "serial-usart-stm32"))]
compile_error!("Select only one serial backend feature");

#[cfg(not(any(feature = "serial-uart16550", feature = "serial-usart-stm32")))]
compile_error!("Select a serial backend feature: serial-uart16550 or serial-usart-stm32");

struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        puts(s);
        Ok(())
    }
}

pub fn write_fmt(args: fmt::Arguments<'_>) {
    let mut writer = SerialWriter;
    if fmt::write(&mut writer, args).is_err() {
        unreachable!("serial writer cannot fail");
    }
}
