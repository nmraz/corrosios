#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use mm::physmap;
use mm::types::PhysAddr;

mod arch;
mod mm;
mod panic;

#[no_mangle]
fn kernel_main(bootinfo_paddr: PhysAddr) -> ! {
    let bootinfo = unsafe { physmap::map_bootinfo(bootinfo_paddr) };
    arch::irq::idle_loop();
}
