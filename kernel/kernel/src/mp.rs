use core::marker::PhantomData;

use alloc::boxed::Box;
use log::debug;

use crate::arch;
use crate::mm::vm;
use crate::sync::irq::IrqDisabled;

pub struct PerCpu {
    pub cpu_num: u32,
    pub vm: vm::PerCpu,
    _not_send_sync: PhantomData<*const ()>,
}

/// Retrieves the per-CPU structure for the current processor.
pub fn current_percpu(irq_disabled: &IrqDisabled) -> &PerCpu {
    unsafe { &*arch::cpu::current_percpu(irq_disabled).cast() }
}

/// Initializes the bootstrap processor (BSP), including early interrupt handlers and per-CPU data.
///
/// # Safety
///
/// * This function must be called only once on the BSP.
/// * This function must be called in a consistent state where it is valid to enable interrupts.
///   In particular, there should be no spinlocks held and `irq_disabled` should be the only live
///   instance of [`IrqDisabled`].
pub unsafe fn init_bsp(irq_disabled: IrqDisabled) {
    let percpu = Box::try_new(PerCpu {
        cpu_num: 0,
        vm: vm::PerCpu::new(),
        _not_send_sync: PhantomData,
    })
    .expect("failed to allocate initial per-CPU structure");

    debug!("allocated common percpu at {:p}", percpu);

    let percpu = Box::leak(percpu);
    unsafe {
        arch::cpu::init_bsp(percpu as *const _ as *const (), irq_disabled);
    }
}
