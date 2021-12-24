#![feature(asm, global_asm, asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

mod arch;
mod panic;

#[no_mangle]
fn kernel_main(bootinfo_paddr: usize) -> ! {
    arch::irq::idle_loop();
}
