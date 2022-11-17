use core::arch::asm;
use core::mem;

use log::debug;

use crate::arch::x86_64::x64_cpu::{write_cr4, write_ia32_efer, Cr4, Ia32Efer};
use crate::mp::PerCpu;
use crate::sync::irq::IrqDisabled;

use super::descriptor::{get_idt, get_idt_size, init_idt, Gdt, KERNEL_CODE_SELECTOR, TSS_SELECTOR};
use super::percpu;
use super::x64_cpu::{
    cli, get_rflags, hlt, lgdt, lidt, lldt, ltr, read_cr0, read_cr4, read_ia32_efer, sti,
    write_cr0, Cr0, DescriptorRegister, Rflags,
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

pub fn current_percpu(irq_disabled: &IrqDisabled) -> &PerCpu {
    percpu::current_common(irq_disabled)
}

pub unsafe fn init_bsp(common_percpu: &'static PerCpu, irq_disabled: IrqDisabled) {
    init_idt();
    unsafe {
        init_current(common_percpu, irq_disabled);
    }
}

pub unsafe fn init_current(common_percpu: &'static PerCpu, irq_disabled: IrqDisabled) {
    unsafe {
        let cur_percpu = percpu::init_current(common_percpu, &irq_disabled);
        debug!("initialized arch percpu at {:p}", cur_percpu);
        load_gdt(&cur_percpu.gdt);
        load_idt();
        init_cpu_features();

        // Everything is ready, enable interrupts now
        sti();
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

unsafe fn init_cpu_features() {
    let mut cr0 = read_cr0();
    cr0 |= Cr0::WP;
    unsafe {
        write_cr0(cr0);
    }

    let mut cr4 = read_cr4();
    cr4 |= Cr4::OSFXCR | Cr4::OSXMMEXCPT;
    unsafe {
        write_cr4(cr4);
    }

    let mut ia32_efer = read_ia32_efer();
    ia32_efer |= Ia32Efer::NXE | Ia32Efer::SCE;
    unsafe {
        write_ia32_efer(ia32_efer);
    }
}
