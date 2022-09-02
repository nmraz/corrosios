#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

use arch::cpu;
use mm::types::PhysAddr;

mod arch;
#[macro_use]
mod console;
mod kimage;
mod mm;
mod panic;
mod sync;

#[no_mangle]
extern "C" fn kernel_main(
    kernel_paddr: PhysAddr,
    bootinfo_paddr: PhysAddr,
    bootinfo_size: usize,
) -> ! {
    arch::earlyconsole::init_install();

    unsafe {
        kimage::init(kernel_paddr);
    }

    println!(
        "kernel loaded at {}-{}, mapped at {}-{}",
        kimage::phys_base().addr(),
        kimage::phys_end().addr(),
        kimage::virt_base().addr(),
        kimage::virt_end().addr()
    );

    println!("bootinfo at {}, size {:#x}", bootinfo_paddr, bootinfo_size);

    println!("initializing memory manager");
    unsafe { mm::init(bootinfo_paddr, bootinfo_size) };
    println!("memory manager initialized");

    for _ in 0..8 {
        mm::pmm::dump_usage();
        let pfn = mm::pmm::allocate(0).expect("failed to allocate frame");
        println!("allocated {}", pfn);
    }

    mm::pmm::dump_usage();

    cpu::halt();
}
