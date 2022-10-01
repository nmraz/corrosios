use core::fmt::{Arguments, Write};

use crate::arch::serial::Console;
use crate::sync::SpinLock;

macro_rules! println {
    () => {
        println!("")
    };

    ($($args:tt)*) => {
        $crate::console::writeln_fmt(format_args!($($args)*))
    };
}

static CONSOLE: SpinLock<Option<Console>> = SpinLock::new(None);

pub fn init() {
    CONSOLE.with(|console, _| {
        assert!(console.is_none());
        unsafe {
            *console = Some(Console::new());
        }
    });
}

pub fn writeln_fmt(args: Arguments<'_>) {
    CONSOLE.with(|console, _| {
        if let Some(console) = console {
            let _ = writeln!(console, "{}", args);
        }
    })
}
