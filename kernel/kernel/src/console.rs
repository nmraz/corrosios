use core::fmt::{Arguments, Write};
use core::sync::atomic::Ordering;

use arrayvec::ArrayString;
use atomic_ref::AtomicRef;

macro_rules! print {
    ($($args:tt)*) => {
        $crate::console::write_fmt(format_args!($($args)*))
    };
}

macro_rules! println {
    () => {
        println!("")
    };

    ($($args:tt)*) => {
        $crate::console::writeln_fmt(format_args!($($args)*))
    };
}

pub trait Console {
    fn write(&self, msg: &str);
}

pub struct ConsoleDesc {
    pub console: &'static (dyn Console + Sync),
}

pub fn set_console(console: &'static ConsoleDesc) {
    CONSOLE.store(Some(console), Ordering::Release);
}

pub fn write(msg: &str) {
    if let Some(console) = CONSOLE.load(Ordering::Acquire) {
        console.console.write(msg);
    }
}

pub fn write_fmt(args: Arguments<'_>) {
    let mut msg = ArrayString::<512>::new();
    if write!(msg, "{}", args).is_ok() {
        write(&msg);
    }
}

pub fn writeln_fmt(args: Arguments<'_>) {
    let mut msg = ArrayString::<512>::new();
    if writeln!(msg, "{}", args).is_ok() {
        write(&msg);
    }
}

static CONSOLE: AtomicRef<'static, ConsoleDesc> = AtomicRef::new(None);
