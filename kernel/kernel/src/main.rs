#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use bootinfo::item::Framebuffer;
use bootinfo::view::View;
use bootinfo::ItemKind;
use mm::physmap;
use mm::types::PhysAddr;

use crate::mm::physmap::paddr_to_physmap;

mod arch;
#[macro_use]
mod console;
mod mm;
mod panic;

#[no_mangle]
fn kernel_main(bootinfo_paddr: PhysAddr) -> ! {
    arch::earlyconsole::init_install();

    println!("corrosios starting");

    unsafe { physmap::init(bootinfo_paddr) };

    let bootinfo = unsafe { View::new(&*paddr_to_physmap(bootinfo_paddr).as_ptr()) }.unwrap();
    for item in bootinfo.items() {
        if item.kind() == ItemKind::FRAMEBUFFER {
            let framebuffer = unsafe { item.get::<Framebuffer>() }.unwrap();
            println!("framebuffer: {:#x?}", framebuffer);
        }
    }

    arch::irq::idle_loop();
}
