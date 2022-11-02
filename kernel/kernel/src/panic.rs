use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::cpu;

#[panic_handler]
fn handle_panic(info: &PanicInfo<'_>) -> ! {
    if !PANICKING.swap(true, Ordering::Relaxed) {
        println!("\n************ KERNEL PANIC ************");

        if let Some(message) = info.message() {
            println!("{}", message);
        }

        if let Some(location) = info.location() {
            println!("\nat {}", location);
        }

        println!("**************************************\n");
    }

    cpu::halt();
}

static PANICKING: AtomicBool = AtomicBool::new(false);
