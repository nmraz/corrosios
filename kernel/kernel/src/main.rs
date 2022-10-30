#![feature(alloc_error_handler, allocator_api)]
#![feature(new_uninit)]
#![feature(asm_const)]
#![feature(panic_info_message)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use core::{mem, slice};

use arch::cpu;
use bootinfo::item::Framebuffer;
use bootinfo::view::View;
use bootinfo::ItemKind;
use log::{debug, info};
use mm::types::PhysAddr;
use num_utils::div_ceil;

use crate::arch::mmu::PAGE_SIZE;
use crate::mm::physmap::paddr_to_physmap;
use crate::mm::types::{CacheMode, Protection};
use crate::mm::vm::kernel_aspace::iomap;
use crate::sync::irq::IrqDisabled;

#[macro_use]
mod console;

mod arch;
mod err;
mod global_alloc;
mod kimage;
mod logging;
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
    logging::init();

    info!("corrosios starting");

    unsafe {
        kimage::init(kernel_paddr);
    }

    debug!(
        "kernel loaded at {}-{}, mapped at {}-{}",
        kimage::phys_base().addr(),
        kimage::phys_end().addr(),
        kimage::virt_base().addr(),
        kimage::virt_end().addr()
    );

    debug!("bootinfo at {}, size {:#x}", bootinfo_paddr, bootinfo_size);

    info!("initializing memory manager");
    unsafe {
        mm::init(bootinfo_paddr, bootinfo_size, &irq_disabled);
    }
    info!("memory manager initialized");

    unsafe {
        arch::cpu::init_bsp(irq_disabled);
    }

    mm::pmm::dump_usage();

    debug!("triggering IRQ 55");
    unsafe {
        core::arch::asm!("int 55");
    }

    let bootinfo_slice =
        unsafe { slice::from_raw_parts(paddr_to_physmap(bootinfo_paddr).as_ptr(), bootinfo_size) };

    let bootinfo = View::new(bootinfo_slice).expect("bad bootinfo");

    let framebuffer_item = bootinfo
        .items()
        .find(|item| item.kind() == ItemKind::FRAMEBUFFER)
        .expect("no framebuffer");

    let framebuffer_desc: &Framebuffer =
        unsafe { framebuffer_item.get() }.expect("framebuffer info invalid");

    let framebuffer_paddr = PhysAddr::new(framebuffer_desc.paddr);

    debug!(
        "framebuffer: phys range {}-{}, dimensions {}x{}, format {:?}",
        framebuffer_paddr,
        framebuffer_paddr + framebuffer_desc.size,
        framebuffer_desc.width,
        framebuffer_desc.height,
        framebuffer_desc.format
    );

    let framebuffer_mapping = unsafe {
        iomap(
            framebuffer_paddr.containing_frame(),
            div_ceil(framebuffer_desc.size, PAGE_SIZE),
            Protection::READ | Protection::WRITE,
            CacheMode::WriteCombining,
        )
    }
    .expect("failed to map framebuffer");

    debug!("framebuffer mapped at {}", framebuffer_mapping.addr());

    let framebuffer_slice: &mut [u32] = unsafe {
        slice::from_raw_parts_mut(
            framebuffer_mapping.addr().as_mut_ptr(),
            framebuffer_desc.size / mem::size_of::<u32>(),
        )
    };

    debug!("writing to framebuffer");

    for row in 0..framebuffer_desc.height {
        for col in 0..framebuffer_desc.width {
            framebuffer_slice[(row * framebuffer_desc.stride + col) as usize] = 0xff;
        }
    }

    debug!("causing irrecoverable page fault");
    unsafe {
        *(0x1234 as *mut u64) = 0;
    }

    cpu::halt();
}
