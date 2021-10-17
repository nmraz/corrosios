#![feature(asm, global_asm)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod arch;

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[link_section = ".boottext"]
#[no_mangle]
fn boot_main() -> ! {
    loop {}
}
