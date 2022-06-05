#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use arch::cpu;
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

    unsafe { physmap::init(bootinfo_paddr) };

    let bootinfo = unsafe { View::new(paddr_to_physmap(bootinfo_paddr).as_ptr()) }.unwrap();

    let mem_map_view = bootinfo
        .items()
        .find(|item| item.kind() == ItemKind::MEMORY_MAP)
        .unwrap();
    let mem_map = unsafe { mem_map_view.get_slice() }.unwrap();

    unsafe {
        mm::pmm::init(mem_map);
    }

    cpu::halt();
}
