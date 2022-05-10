use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

#[panic_handler]
fn handle_panic(info: &PanicInfo<'_>) -> ! {
    if PANICKING.swap(true, Ordering::Relaxed) {
        loop {}
    }

    println!("kernel panic: {}", info);
    loop {}
}

static PANICKING: AtomicBool = AtomicBool::new(false);
