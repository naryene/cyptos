use core::ptr::{read_volatile, write_volatile};

// The base memory address of the UART peripheral. All register accesses are
// offsets from this address. 0x1000_0000 is the UART address on QEMU's RISC-V virt machine.
const UART_BASE: usize = 0x1000_0000;

// Transmit Holding Register — write a byte here to send it out over the serial line.
// Only valid when DLAB = 0.
const THR: usize = 0x00;

// Receive Buffer Register — read a byte here to get incoming serial data.
// Shares the same offset as THR (0x00); hardware tells them apart by read vs write.
// Only valid when DLAB = 0.
const RBR: usize = 0x00;

// Interrupt Enable Register — each bit enables a specific UART interrupt.
// We write 0x00 here during init to disable all interrupts and use polling instead.
// Only valid when DLAB = 0.
const IER: usize = 0x01;

// FIFO Control Register — controls the internal transmit/receive buffers (FIFOs).
// Writing 0x07 here enables the FIFOs and clears any leftover data in them.
const FCR: usize = 0x02;

// Line Control Register — configures the serial format (word length, stop bits, parity).
// Bit 7 of this register is the special DLAB flag (see LCR_DLAB below).
const LCR: usize = 0x03;

// Line Status Register — a read-only register that reports the current state of the UART.
// Individual bits tell you whether data is ready to receive, or if the transmitter is free.
const LSR: usize = 0x05;

// Bit 5 of LSR — Transmit Empty flag.
// When this bit is 1, the transmit buffer is ready to accept a new byte to send.
// We spin on this bit in putc() before writing to THR.
const LSR_TX_EMPTY: u8 = 1 << 5; // 0b00100000 = 0x20

// Bit 0 of LSR — Receive Ready flag.
// When this bit is 1, a byte has arrived and is waiting to be read from RBR.
// We spin on this bit in getc() before reading from RBR.
const LSR_RX_READY: u8 = 1 << 0; // 0b00000001 = 0x01

// LCR value for 8-bit word length, no parity, 1 stop bit (the most common serial format).
// Bits [1:0] of LCR control word length: 0b11 = 8 bits.
// This also clears the DLAB bit (bit 7 = 0), switching back to normal register mode.
const LCR_8BIT: u8 = 0b11;

// Bit 7 of LCR — Divisor Latch Access Bit.
// When set to 1, offsets 0x00 and 0x01 switch from THR/RBR/IER to the baud rate
// divisor registers (DLL and DLH), allowing you to configure the baud rate.
// Must be cleared back to 0 before normal data transmission/reception.
const LCR_DLAB: u8 = 1 << 7; // 0b10000000 = 0x80

pub fn init() {
    unsafe {
        let base = UART_BASE as *mut u8;
        write_volatile(base.add(IER), 0x00);
        write_volatile(base.add(LCR), LCR_DLAB);
        write_volatile(base.add(0), 0x01);
        write_volatile(base.add(1), 0x00);
        write_volatile(base.add(LCR), LCR_8BIT);
        write_volatile(base.add(FCR), 0x07);
    }
}

pub fn putc(c: u8) {
    unsafe {
        let base = UART_BASE as *mut u8;
        while (read_volatile(base.add(LSR)) & LSR_TX_EMPTY) == 0 {}
        write_volatile(base.add(THR), c);
    }
}

pub fn puts(s: &str) {
    for c in s.bytes() {
        if c == b'\n' {
            putc(b'\r');
        }
        putc(c);
    }
}

pub fn put_dec(mut n: u64) {
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut idx = 0;
    while n > 0 {
        buf[idx] = b'0' + (n % 10) as u8;
        n /= 10;
        idx += 1;
    }
    while idx > 0 {
        idx -= 1;
        putc(buf[idx]);
    }
}

#[allow(dead_code)]
pub fn put_hex(mut n: u64) {
    puts("0x");
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while n > 0 {
        let d = (n & 0xF) as u8;
        buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        n >>= 4;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        putc(buf[i]);
    }
}

#[allow(dead_code)]
pub fn getc() -> u8 {
    unsafe {
        let base = UART_BASE as *mut u8;
        while (read_volatile(base.add(LSR)) & LSR_RX_READY) == 0 {}
        read_volatile(base.add(RBR))
    }
}
