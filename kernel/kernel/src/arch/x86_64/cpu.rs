use core::arch::asm;
use core::mem;

use crate::sync::irq::IrqDisabled;

use super::descriptor::{Gdt, KERNEL_CODE_SELECTOR, TSS_SELECTOR};
use super::percpu;
use super::x64_cpu::{cli, get_rflags, hlt, lgdt, lldt, ltr, sti, DescriptorRegister};

pub unsafe fn init(irq_disabled: IrqDisabled) {
    unsafe {
        let cur_percpu = percpu::init_current(&irq_disabled);
        println!("initialized percpu at {:p}", cur_percpu);
        load_gdt(&cur_percpu.gdt);
    }
}

unsafe fn load_gdt(gdt: &Gdt) {
    unsafe {
        let desc = DescriptorRegister {
            limit: (mem::size_of::<Gdt>() - 1) as u16,
            ptr: gdt as *const _ as u64,
        };

        lgdt(&desc);

        load_kernel_cs();
        ltr(TSS_SELECTOR);
        lldt(0);
    }
}

unsafe fn load_kernel_cs() {
    unsafe {
        asm!(
            "push {KERNEL_CODE_SELECTOR}",
            "lea {scratch}, [rip + 1f]",
            "push {scratch}",
            "retfq",
            "1: nop",
            KERNEL_CODE_SELECTOR = const KERNEL_CODE_SELECTOR,
            scratch = out(reg) _,
        );
    }
}

#[inline]
pub fn halt() -> ! {
    unsafe {
        cli();
    }
    loop {
        hlt();
    }
}

pub fn irq_enabled() -> bool {
    get_rflags() & 0x200 != 0
}

#[inline]
pub unsafe fn disable_irq() {
    unsafe {
        cli();
    }
}

#[inline]
pub unsafe fn enable_irq() {
    unsafe {
        sti();
    }
}
