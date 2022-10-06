#![feature(alloc_error_handler, allocator_api)]
#![feature(asm_const)]
#![feature(panic_info_message)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use arch::cpu;
use mm::types::PhysAddr;

use crate::sync::irq::IrqDisabled;

#[macro_use]
mod console;

mod arch;
mod err;
mod global_alloc;
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

    unsafe {
        arch::cpu::init_bsp(irq_disabled);
    }

    mm::pmm::dump_usage();

    println!("triggering IRQ 55");
    unsafe {
        core::arch::asm!("int 55");
    }

    println!("causing irrecoverable page fault");
    unsafe {
        *(0x1234 as *mut u64) = 0;
    }

    cpu::halt();
}
