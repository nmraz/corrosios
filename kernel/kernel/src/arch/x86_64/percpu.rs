use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};

use crate::mm::heap;
use crate::mm::types::VirtAddr;
use crate::sync::irq::IrqDisabled;

use super::descriptor::{Gdt, Tss};
use super::x64_cpu::{read_gs_qword, wrgsbase};

#[repr(C, align(0x10))]
pub struct InterruptStack(UnsafeCell<MaybeUninit<[u8; 0x1000]>>);

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
    inner: X64PerCpu,
}

pub fn current(_irq_disabled: &IrqDisabled) -> &X64PerCpu {
    unsafe {
        // Note: offset 0 is guaranteed to be the `ptr` field of `X64PerCpuWrapper`
        let ptr = read_gs_qword::<0>() as *const X64PerCpuWrapper;
        &(*ptr).inner
    }
}

pub unsafe fn init_current(_irq_disabled: &IrqDisabled) -> &X64PerCpu {
    let wrapper: *mut X64PerCpuWrapper = heap::allocate(Layout::new::<X64PerCpuWrapper>())
        .expect("failed to allocate per-CPU structure")
        .as_ptr()
        .cast();

    let null_vaddr = VirtAddr::new(0);

    unsafe {
        addr_of_mut!((*wrapper).ptr).write(wrapper as *const _);

        let inner = addr_of_mut!((*wrapper).inner);
        let nmi_stack = VirtAddr::from_ptr(addr_of!((*inner).nmi_stack));
        let double_fault_stack = VirtAddr::from_ptr(addr_of!((*inner).double_fault_stack));

        let tss = UnsafeCell::raw_get(addr_of_mut!((*inner).tss));
        Tss::init(
            tss,
            nmi_stack,
            double_fault_stack,
            null_vaddr,
            null_vaddr,
            null_vaddr,
            null_vaddr,
            null_vaddr,
        );

        let gdt = addr_of_mut!((*inner).gdt);
        gdt.write(Gdt::new(VirtAddr::from_ptr(tss)));

        wrgsbase(VirtAddr::from_ptr(wrapper));
        &*inner
    }
}
