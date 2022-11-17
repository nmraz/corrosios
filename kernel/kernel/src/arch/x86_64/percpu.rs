use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};

use crate::mm::heap;
use crate::mm::types::VirtAddr;
use crate::mp::PerCpu;
use crate::sync::irq::IrqDisabled;

use super::descriptor::{Gdt, Tss};
use super::x64_cpu::{read_gs_qword, wrgsbase};

const INTERRUPT_STACK_SIZE: usize = 0x2000;

#[repr(C, align(0x10))]
pub struct InterruptStack(UnsafeCell<MaybeUninit<[u8; INTERRUPT_STACK_SIZE]>>);

pub struct X64PerCpu {
    pub tss: UnsafeCell<Tss>,
    pub gdt: Gdt,
    pub nmi_stack: InterruptStack,
    pub double_fault_stack: InterruptStack,
}

#[repr(C)]
struct X64PerCpuWrapper {
    /// Direct pointer back to this structure, to allow cheap gs-relative access.
    /// This field must reside at offset 0 of the structure.
    ptr: *const X64PerCpuWrapper,
    /// Pointer to the common (architecture-independent) per-cpu structure.
    common_ptr: *const PerCpu,
    inner: X64PerCpu,
}

const PERCPU_PTR_OFFSET: usize = 0;
const PERCPU_COMMON_PTR_OFFSET: usize = 8;

pub fn current_x64(_irq_disabled: &IrqDisabled) -> &X64PerCpu {
    unsafe {
        // Note: offset 0 is guaranteed to be the `ptr` field of `X64PerCpuWrapper`
        let ptr = read_gs_qword::<PERCPU_PTR_OFFSET>() as *const X64PerCpuWrapper;
        &(*ptr).inner
    }
}

pub fn current_common(_irq_disabled: &IrqDisabled) -> &PerCpu {
    unsafe { &*(read_gs_qword::<PERCPU_COMMON_PTR_OFFSET>() as *const _) }
}

pub unsafe fn init_current<'a>(
    common_percpu: &'static PerCpu,
    _irq_disabled: &'a IrqDisabled,
) -> &'a X64PerCpu {
    let wrapper: *mut X64PerCpuWrapper = heap::allocate(Layout::new::<X64PerCpuWrapper>())
        .expect("failed to allocate architecture per-CPU structure")
        .as_ptr()
        .cast();

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
        &*inner
    }
}
