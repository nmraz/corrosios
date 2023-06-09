use core::marker::PhantomData;

use spin_once::TakeOnce;

use crate::mm::vm;
use crate::sync::irq::IrqDisabled;
use crate::sync::resched::ReschedDisabled;
use crate::{arch, sched};

pub struct PerCpu {
    pub cpu_num: u32,
    pub vm: vm::PerCpu,
    pub sched: sched::CpuState,
    _not_send_sync: PhantomData<*const ()>,
}

impl PerCpu {
    fn new(cpu_num: u32) -> Self {
        Self {
            cpu_num,
            vm: vm::PerCpu::new(),
            sched: sched::CpuState::new(),
            _not_send_sync: PhantomData,
        }
    }
}

/// Retrieves the per-CPU structure for the current processor.
pub fn current_percpu(_resched_disabled: &ReschedDisabled) -> &PerCpu {
    unsafe { &*arch::cpu::current_percpu().cast() }
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
        .take_init(PerCpu::new(0))
        .expect("BSP percpu already initialized");

    unsafe {
        arch::cpu::init_bsp_early(percpu as *const _ as *const (), irq_disabled);
    }
}
