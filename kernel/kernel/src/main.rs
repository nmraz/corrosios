#![feature(asm, global_asm)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod arch;

#[no_mangle]
fn kernel_main(bootinfo_paddr: usize) -> ! {
    arch::irq::idle_loop();
}

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {}
}
