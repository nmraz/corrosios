use crate::sync::irq::IrqDisabled;

use super::descriptor::Tss;
use super::x64_cpu::read_gs_qword;

pub struct Gdt;

pub struct X64PerCpu {
    pub tss: Tss,
    pub gdt: Gdt,
}

pub fn current(_irq_disabled: &IrqDisabled) -> &X64PerCpu {
    unsafe {
        // Note: offset 0 is guaranteed to be the `ptr` field of `X64PerCpuWrapper`
        let ptr = read_gs_qword::<0>() as *const X64PerCpuWrapper;
        &(*ptr).inner
    }
}

#[repr(C)]
struct X64PerCpuWrapper {
    /// Direct pointer back to this structure, to allow cheap gs-relative access.
    /// This field must reside at offset 0 of the structure.
    ptr: *const X64PerCpuWrapper,
    inner: X64PerCpu,
}
