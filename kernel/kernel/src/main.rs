#![feature(alloc_error_handler, allocator_api)]
#![feature(new_uninit)]
#![feature(asm_const)]
#![feature(panic_info_message)]
#![feature(utf8_chunks)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use core::{mem, slice};

use arch::cpu;
use log::{debug, info};
use mm::types::PhysAddr;
use num_utils::div_ceil;

use crate::arch::mmu::PAGE_SIZE;
use crate::bootparse::BootinfoData;
use crate::mm::types::{CacheMode, Protection};
use crate::mm::vm::kernel_aspace::iomap;
use crate::sync::irq::IrqDisabled;

#[macro_use]
mod console;

mod arch;
mod bootparse;
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

    unsafe {
        // This needs to happen first, before we start calling general Rust code.
        kimage::init(kernel_paddr);
    }

    // Get a physmap set up so we can parse serial/logging options.
    let mm_init_ctx = unsafe { mm::init_early(bootinfo_paddr, bootinfo_size, &irq_disabled) };

    // Safety: we have just set up the physmap and trust the loader.
    let bootinfo = unsafe { BootinfoData::parse(bootinfo_paddr, bootinfo_size) };

    console::init();
    logging::init();

    info!("corrosios starting");

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
        mm::init_late(mm_init_ctx, &bootinfo, &irq_disabled);
    }
    info!("memory manager initialized");

    unsafe {
        arch::cpu::init_bsp(irq_disabled);
    }

    debug!("triggering IRQ 55");
    unsafe {
        core::arch::asm!("int 55");
    }

    info!("kernel command line: {}", bootinfo.command_line());

    if let Some(framebuffer_info) = bootinfo.framebuffer_info() {
        let framebuffer_paddr = PhysAddr::new(framebuffer_info.paddr);

        debug!(
            "framebuffer: phys range {}-{}, dimensions {}x{}, format {:?}",
            framebuffer_paddr,
            framebuffer_paddr + framebuffer_info.byte_size,
            framebuffer_info.pixel_width,
            framebuffer_info.pixel_height,
            framebuffer_info.pixel_format
        );

        let framebuffer_mapping = unsafe {
            iomap(
                framebuffer_paddr.containing_frame(),
                div_ceil(framebuffer_info.byte_size, PAGE_SIZE),
                Protection::READ | Protection::WRITE,
                CacheMode::WriteCombining,
            )
        }
        .expect("failed to map framebuffer");

        debug!("framebuffer mapped at {}", framebuffer_mapping.addr());

        let framebuffer_slice: &mut [u32] = unsafe {
            slice::from_raw_parts_mut(
                framebuffer_mapping.addr().as_mut_ptr(),
                framebuffer_info.byte_size / mem::size_of::<u32>(),
            )
        };

        debug!("writing to framebuffer");

        for row in 0..framebuffer_info.pixel_height {
            for col in 0..framebuffer_info.pixel_width {
                framebuffer_slice[(row * framebuffer_info.pixel_stride + col) as usize] = 0xff;
            }
        }
    }

    mm::pmm::dump_usage();

    debug!("attempting to write to kernel code");
    unsafe {
        extern "C" {
            static mut __code_start: u8;
        }

        __code_start = 0xab;
    }

    cpu::halt();
}
