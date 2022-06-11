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
mod kimage;
mod mm;
mod panic;

#[no_mangle]
extern "C" fn kernel_main(kernel_paddr: PhysAddr, bootinfo_paddr: PhysAddr) -> ! {
    arch::earlyconsole::init_install();

    unsafe {
        kimage::init(kernel_paddr);
    }

    println!(
        "kernel loaded at {:#x}-{:#x}, mapped at {:#x}-{:#x}",
        kimage::phys_base().addr().as_usize(),
        (kimage::phys_base() + kimage::total_pages())
            .addr()
            .as_usize(),
        kimage::virt_base().addr().as_usize(),
        kimage::virt_end().addr().as_usize()
    );

    unsafe {
        physmap::init(bootinfo_paddr);
    }

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
