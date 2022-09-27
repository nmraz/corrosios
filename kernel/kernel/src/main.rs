#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use arch::cpu;
use mm::types::PhysAddr;

use crate::sync::irq::IrqDisabled;

mod arch;
#[macro_use]
mod console;
mod global_alloc;
mod kimage;
mod mm;
mod panic;
mod sync;

#[derive(Debug, Clone, Copy)]
#[repr(align(512))]
struct AlignedU32(u32);

#[no_mangle]
extern "C" fn kernel_main(
    kernel_paddr: PhysAddr,
    bootinfo_paddr: PhysAddr,
    bootinfo_size: usize,
) -> ! {
    // Safety: main is called with interrupts disabled.
    let irq_disabled = unsafe { IrqDisabled::new() };

    console::init();

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
    unsafe {
        mm::init(bootinfo_paddr, bootinfo_size, &irq_disabled);
    }
    println!("memory manager initialized");

    mm::heap::dump_size_classes();

    let x = Box::new(AlignedU32(5));
    let y = Box::new(AlignedU32(7));
    println!("x: {:?} ({:p}), y: {:?} ({:p})", *x, x, *y, y);

    mm::pmm::dump_usage();

    cpu::halt();
}
