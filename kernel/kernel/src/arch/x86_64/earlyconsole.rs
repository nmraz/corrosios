use core::hint;

use bitflags::bitflags;

use crate::console::{self, Console, ConsoleDesc};

use super::ioport::{inb, outb};

pub fn init_install() {
    init();
    install();
}

pub fn init() {
    set_divisor(1);
    set_line_control(LineControlFlags::WORD_LENGTH_8);
    set_fifo_control(0);
    unsafe { set_interrupt_enable(0) };
    set_modem_control(ModemControlFlags::DATA_TERMINAL_READY | ModemControlFlags::REQUEST_TO_SEND);
}

pub fn write(data: &[u8]) {
    for &byte in data {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}

fn write_byte(byte: u8) {
    while !get_line_status().contains(LineStatus::EMPTY_THR) {
        hint::spin_loop();
    }

    unsafe {
        write_reg(THR_OFF, byte);
    }
}

fn install() {
    console::set_console(&EARLY_CONSOLE_DESC);
}

struct EarlyConsole;

impl Console for EarlyConsole {
    fn write(&self, msg: &str) {
        write(msg.as_bytes())
    }
}

static EARLY_CONSOLE: EarlyConsole = EarlyConsole;
static EARLY_CONSOLE_DESC: ConsoleDesc = ConsoleDesc {
    console: &EARLY_CONSOLE,
};

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

fn set_divisor(divisor: u16) {
    set_line_control(LineControlFlags::DLAB);

    unsafe {
        write_reg(DIVISOR_LOW_OFF, (divisor & 0xff) as u8);
        write_reg(DIVISOR_HIGH_OFF, (divisor >> 8) as u8);
    }

    set_line_control(LineControlFlags::empty());
}

fn set_fifo_control(val: u8) {
    unsafe { write_reg(FCR_OFF, val) };
}

unsafe fn set_interrupt_enable(enable: u8) {
    unsafe { write_reg(IER_OFF, enable) };
}

fn set_line_control(flags: LineControlFlags) {
    unsafe { write_reg(LCR_OFF, flags.bits()) };
}

fn set_modem_control(flags: ModemControlFlags) {
    unsafe { write_reg(MCR_OFF, flags.bits()) }
}

fn get_line_status() -> LineStatus {
    unsafe { LineStatus::from_bits_unchecked(read_reg(LSR_OFF)) }
}

unsafe fn write_reg(off: u16, val: u8) {
    unsafe { outb(REGISTER_BASE + off, val) }
}

unsafe fn read_reg(off: u16) -> u8 {
    unsafe { inb(REGISTER_BASE + off) }
}

const REGISTER_BASE: u16 = 0x3f8;

const IER_OFF: u16 = 1;
const FCR_OFF: u16 = 2;

const THR_OFF: u16 = 0;

const DIVISOR_LOW_OFF: u16 = 0;
const DIVISOR_HIGH_OFF: u16 = 1;

const LCR_OFF: u16 = 3;
const MCR_OFF: u16 = 4;
const LSR_OFF: u16 = 5;
