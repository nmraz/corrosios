use core::marker::PhantomData;

use spin_once::TakeOnce;

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

/// Performs early initialization of the bootstrap processor (BSP), including early interrupt
/// handlers and per-CPU data.
///
/// This function should be called very early (before general-purpose Rust code runs), as such code
/// may indirectly require per-CPU data.
///
/// # Safety
///
/// * This function must be called only once on the BSP.
pub unsafe fn init_bsp_early(irq_disabled: &IrqDisabled) {
    static BSP_PERCPU: TakeOnce<PerCpu> = TakeOnce::new();
    let percpu = BSP_PERCPU
        .take_init(PerCpu {
            cpu_num: 0,
            vm: vm::PerCpu::new(),
            _not_send_sync: PhantomData,
        })
        .expect("BSP percpu already initialized");

    unsafe {
        arch::cpu::init_bsp_early(percpu as *const _ as *const (), irq_disabled);
    }
}
