use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::cpu;

#[panic_handler]
fn handle_panic(info: &PanicInfo<'_>) -> ! {
    if !PANICKING.swap(true, Ordering::Relaxed) {
        println!("kernel panic: {}", info);
    }

    cpu::halt();
}

static PANICKING: AtomicBool = AtomicBool::new(false);
