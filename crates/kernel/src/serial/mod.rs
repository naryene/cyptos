#![allow(unused_imports)]
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
