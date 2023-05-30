use core::arch::asm;
use core::mem;

use crate::sync::irq::IrqDisabled;

use super::descriptor::{get_idt, get_idt_size, init_idt, Gdt, KERNEL_CODE_SELECTOR, TSS_SELECTOR};
use super::percpu;
use super::x64_cpu::{
    cli, get_rflags, hlt, lgdt, lidt, lldt, ltr, sti, DescriptorRegister, Rflags,
};

#[inline]
pub fn halt() -> ! {
    unsafe {
        cli();
    }
    loop {
        hlt();
    }
}

pub fn idle_loop() -> ! {
    loop {
        hlt();
    }
}

pub fn irq_enabled() -> bool {
    get_rflags().contains(Rflags::IF)
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

pub fn current_percpu(irq_disabled: &IrqDisabled) -> *const () {
    percpu::current_common(irq_disabled)
}

pub unsafe fn init_bsp_early(common_percpu: *const (), irq_disabled: &IrqDisabled) {
    init_idt();
    unsafe {
        percpu::init_bsp(common_percpu, irq_disabled);
        finish_init_current_early(irq_disabled);
    }
}

unsafe fn finish_init_current_early(irq_disabled: &IrqDisabled) {
    unsafe {
        let cur_percpu = percpu::current_x64(irq_disabled);
        load_gdt(&cur_percpu.gdt);
        load_idt();
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

unsafe fn load_idt() {
    unsafe {
        let desc = DescriptorRegister {
            limit: (get_idt_size() - 1) as u16,
            ptr: get_idt().as_u64(),
        };
        lidt(&desc);
    }
}
