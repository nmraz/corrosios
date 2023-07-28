use core::cell::{Cell, UnsafeCell};
use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};

use spin_once::TakeOnce;

use crate::mm::types::VirtAddr;
use crate::sync::irq::IrqDisabled;

use super::descriptor::{Gdt, Tss};
use super::x64_cpu::{read_gs_dword, read_gs_qword, wrgsbase, xadd_gs_dword};

const INTERRUPT_STACK_SIZE: usize = 0x2000;

#[repr(C, align(0x10))]
pub struct InterruptStack(UnsafeCell<MaybeUninit<[u8; INTERRUPT_STACK_SIZE]>>);

pub struct X64PerCpu {
    pub tss: UnsafeCell<Tss>,
    pub gdt: Gdt,
    pub nmi_stack: InterruptStack,
    pub double_fault_stack: InterruptStack,
}

#[repr(C, align(64))]
struct X64PerCpuWrapper {
    /// Direct pointer back to this structure, to allow cheap gs-relative access.
    /// This field must reside at offset 0 of the structure.
    ptr: *const X64PerCpuWrapper,
    /// Pointer to the common (architecture-independent) per-cpu structure.
    common_ptr: *const (),
    preempt_blocks: Cell<u32>,
    inner: X64PerCpu,
}

const PERCPU_PTR_OFFSET: usize = 0;
const PERCPU_COMMON_PTR_OFFSET: usize = 8;
const PERCPU_RESCHED_BLOCKS_OFFSET: usize = 0x10;

#[inline]
pub fn current_x64(_irq_disabled: &IrqDisabled) -> &X64PerCpu {
    unsafe { &(*current_wrapper()).inner }
}

#[inline]
pub fn current_common() -> *const () {
    unsafe { read_gs_qword::<PERCPU_COMMON_PTR_OFFSET>() as *const _ }
}

#[inline]
pub fn disable_resched() {
    // This operation doesn't need to be atomic (it is for this core only), but it does need to be
    // a single instruction so that it doesn't get broken up by potential rescheduling interrupts.
    unsafe {
        xadd_gs_dword::<PERCPU_RESCHED_BLOCKS_OFFSET>(1);
    }
}

#[inline]
pub unsafe fn enable_resched() -> u32 {
    unsafe { xadd_gs_dword::<PERCPU_RESCHED_BLOCKS_OFFSET>(-1i32 as u32) - 1 }
}

#[inline]
pub fn resched_disable_count() -> u32 {
    unsafe { read_gs_dword::<PERCPU_RESCHED_BLOCKS_OFFSET>() }
}

pub unsafe fn init_bsp(common_percpu: *const (), irq_disabled: &IrqDisabled) -> &X64PerCpu {
    static BSP_PERCPU: TakeOnce<X64PerCpuWrapper> = TakeOnce::new();
    unsafe {
        &BSP_PERCPU
            .take_init_with(|wrapper| {
                init_current_with(wrapper.as_mut_ptr(), common_percpu, irq_disabled);
            })
            .expect("BSP x64 percpu already initialized")
            .inner
    }
}

unsafe fn init_current_with(
    wrapper: *mut X64PerCpuWrapper,
    common_percpu: *const (),
    _irq_disabled: &IrqDisabled,
) {
    unsafe {
        addr_of_mut!((*wrapper).ptr).write(wrapper as *const _);
        addr_of_mut!((*wrapper).common_ptr).write(common_percpu);

        let inner = addr_of_mut!((*wrapper).inner);
        let nmi_stack = VirtAddr::from_ptr(addr_of!((*inner).nmi_stack).add(1));
        let double_fault_stack = VirtAddr::from_ptr(addr_of!((*inner).double_fault_stack).add(1));

        let tss = UnsafeCell::raw_get(addr_of_mut!((*inner).tss));
        Tss::init(tss, nmi_stack, double_fault_stack);

        let gdt = addr_of_mut!((*inner).gdt);
        gdt.write(Gdt::new(VirtAddr::from_ptr(tss)));

        wrgsbase(VirtAddr::from_ptr(wrapper));
    }
}

unsafe fn current_wrapper() -> *const X64PerCpuWrapper {
    unsafe { read_gs_qword::<PERCPU_PTR_OFFSET>() as *const X64PerCpuWrapper }
}
