#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

mod arch;
mod panic;
mod mm;

#[no_mangle]
fn kernel_main(bootinfo_paddr: usize) -> ! {
    arch::irq::idle_loop();
}
