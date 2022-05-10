#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use bootinfo::item::{Framebuffer, MemoryRange};
use bootinfo::ItemKind;
use mm::physmap;
use mm::types::PhysAddr;

mod arch;
mod console;
mod mm;
mod panic;

#[no_mangle]
fn kernel_main(bootinfo_paddr: PhysAddr) -> ! {
    unsafe { physmap::init(bootinfo_paddr) };

    // for item in bootinfo.items() {
    //     if item.kind() == ItemKind::MEMORY_MAP {
    //         let mmap = unsafe { item.get_slice::<MemoryRange>() }.unwrap();
    //         let len = mmap.len();
    //     }

    //     if item.kind() == ItemKind::FRAMEBUFFER {
    //         let framebuffer = unsafe { item.get::<Framebuffer>() }.unwrap();
    //         let paddr = framebuffer.paddr;
    //     }
    // }

    arch::irq::idle_loop();
}
