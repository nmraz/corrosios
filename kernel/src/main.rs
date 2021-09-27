#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[link_section = ".boottext"]
#[no_mangle]
fn boot_main() -> ! {
    loop {}
}