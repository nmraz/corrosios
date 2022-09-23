use core::hint;

use bitflags::bitflags;

use super::ioport::{inb, outb};

pub struct Console {
    serial: Serial,
}

impl Console {
    /// # Safety
    ///
    /// * Callers should ensure that at most a single instance of `Console` is in use at a given
    ///   time, as it provides (unsynchronized) direct access to the hardware.
    pub unsafe fn new() -> Self {
        unsafe {
            Self {
                serial: Serial::new(0x3f8, 115200),
            }
        }
    }

    pub fn write(&mut self, s: &str) {
        for &byte in s.as_bytes() {
            if byte == b'\n' {
                self.serial.write_byte(b'\r');
            }
            self.serial.write_byte(byte);
        }
    }
}

pub struct Serial {
    base_port: u16,
}

impl Serial {
    /// # Safety
    ///
    /// * `base_port` must indicate an IO port mapping an actual UART controller.
    /// * The specified baud rate must be a valid value supported by the hardware.
    /// * Callers should ensure that at most a single instance of `Serial` is in use for a given
    ///   base port, as this struct enables unsynchronized access to the hardware.
    pub unsafe fn new(base_port: u16, baud: u32) -> Self {
        let mut serial = Self { base_port };
        serial.set_baud(baud);
        serial.set_line_control(LineControlFlags::WORD_LENGTH_8);
        serial.set_fifo_control(0);
        unsafe { serial.set_interrupt_enable(0) };
        serial.set_modem_control(
            ModemControlFlags::DATA_TERMINAL_READY | ModemControlFlags::REQUEST_TO_SEND,
        );
        serial
    }

    pub fn write_byte(&mut self, byte: u8) {
        while !self.get_line_status().contains(LineStatus::EMPTY_THR) {
            hint::spin_loop();
        }

        unsafe {
            self.write_reg(THR_OFF, byte);
        }
    }

    fn set_baud(&mut self, baud: u32) {
        let divisor = (115200 / baud) as u16;

        self.set_line_control(LineControlFlags::DLAB);

        unsafe {
            self.write_reg(DIVISOR_LOW_OFF, (divisor & 0xff) as u8);
            self.write_reg(DIVISOR_HIGH_OFF, (divisor >> 8) as u8);
        }

        self.set_line_control(LineControlFlags::empty());
    }

    fn set_fifo_control(&mut self, val: u8) {
        unsafe { self.write_reg(FCR_OFF, val) };
    }

    unsafe fn set_interrupt_enable(&mut self, enable: u8) {
        unsafe { self.write_reg(IER_OFF, enable) };
    }

    fn set_line_control(&mut self, flags: LineControlFlags) {
        unsafe { self.write_reg(LCR_OFF, flags.bits()) };
    }

    fn set_modem_control(&mut self, flags: ModemControlFlags) {
        unsafe { self.write_reg(MCR_OFF, flags.bits()) }
    }

    fn get_line_status(&mut self) -> LineStatus {
        unsafe { LineStatus::from_bits_unchecked(self.read_reg(LSR_OFF)) }
    }

    unsafe fn write_reg(&mut self, off: u16, val: u8) {
        unsafe { outb(self.base_port + off, val) }
    }

    unsafe fn read_reg(&mut self, off: u16) -> u8 {
        unsafe { inb(self.base_port + off) }
    }
}

bitflags! {
    struct LineControlFlags: u8 {
        const WORD_LENGTH_MASK = 0b11;

        const WORD_LENGTH_5 = 0b00;
        const WORD_LENGTH_6 = 0b01;
        const WORD_LENGTH_7 = 0b10;
        const WORD_LENGTH_8 = 0b11;

        const MORE_STOP_BITS = 1 << 2;
        const PARITY_MASK = 0b111 << 3;
        const BREAK_ENABLE = 1 << 6;
        const DLAB = 1 << 7;
    }
}

bitflags! {
    struct ModemControlFlags: u8 {
        const DATA_TERMINAL_READY = 1 << 0;
        const REQUEST_TO_SEND = 1 << 1;
    }
}

bitflags! {
    struct LineStatus: u8 {
        const DATA_READY = 1 << 0;
        const OVERRUN_ERR = 1 << 1;
        const PARITY_ERR = 1 << 2;
        const FRAMING_ERR = 1 << 3;
        const BREAK_INTERRUPT = 1 << 4;
        const EMPTY_THR = 1 << 5;
        const EMPTY_RBR = 1 << 6;
        const FIFO_ERR = 1 << 7;
    }
}

const IER_OFF: u16 = 1;
const FCR_OFF: u16 = 2;

const THR_OFF: u16 = 0;

const DIVISOR_LOW_OFF: u16 = 0;
const DIVISOR_HIGH_OFF: u16 = 1;

const LCR_OFF: u16 = 3;
const MCR_OFF: u16 = 4;
const LSR_OFF: u16 = 5;
