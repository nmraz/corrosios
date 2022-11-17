use core::marker::PhantomData;

use alloc::boxed::Box;
use log::debug;

use crate::arch;
use crate::sync::irq::IrqDisabled;

pub struct PerCpu {
    pub cpu_num: u32,
    _not_send_sync: PhantomData<*const ()>,
}

pub fn current_percpu(irq_disabled: &IrqDisabled) -> &PerCpu {
    arch::cpu::current_percpu(irq_disabled)
}

pub unsafe fn init_bsp(irq_disabled: IrqDisabled) {
    let percpu = Box::try_new(PerCpu {
        cpu_num: 0,
        _not_send_sync: PhantomData,
    })
    .expect("failed to allocate initial per-CPU structure");

    debug!("allocated common percpu at {:p}", percpu);

    unsafe {
        arch::cpu::init_bsp(Box::leak(percpu), irq_disabled);
    }
}
