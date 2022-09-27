use core::arch::asm;

use crate::mm::types::VirtAddr;

pub const IA32_GS_BASE: u32 = 0xc0000101;

#[repr(C, packed)]
pub struct DescriptorRegister {
    pub limit: u16,
    pub ptr: u64,
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let retval: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") retval, options(nostack));
    }
    retval
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val, options(nostack));
    }
}

#[inline]
pub unsafe fn cli() {
    unsafe {
        asm!("cli", options(nostack));
    }
}

#[inline]
pub unsafe fn sti() {
    unsafe {
        asm!("sti", options(nostack));
    }
}

#[inline]
pub fn hlt() {
    unsafe {
        asm!("hlt", options(nostack));
    }
}

#[inline]
pub fn get_rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!("pushf; pop {}", out(reg) rflags);
    }
    rflags
}

#[inline]
pub fn read_cr3() -> u64 {
    let cr3: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3, options(nostack));
    }
    cr3
}

#[inline]
pub unsafe fn write_cr3(val: u64) {
    unsafe {
        asm!("mov cr3, {}", in(reg) val, options(nostack));
    }
}

#[inline]
pub unsafe fn lgdt(desc: &DescriptorRegister) {
    unsafe {
        asm!("lgdt [{}]", in(reg) desc, options(nostack));
    }
}

#[inline]
pub unsafe fn lldt(desc: &DescriptorRegister) {
    unsafe {
        asm!("lldt [{}]", in(reg) desc, options(nostack));
    }
}

#[inline]
pub unsafe fn lidt(desc: &DescriptorRegister) {
    unsafe {
        asm!("lidt [{}]", in(reg) desc, options(nostack));
    }
}

#[inline]
pub unsafe fn rdmsr(num: u32) -> u64 {
    let eax: u32;
    let edx: u32;

    unsafe {
        asm!("rdmsr", in("ecx") num, out("eax") eax, out("edx") edx, options(nostack));
    }

    ((edx as u64) << 32) | (eax as u64)
}

#[inline]
pub unsafe fn wrmsr(num: u32, val: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") num, in("eax") val as u32, in("edx") (val >> 32) as u32, options(nostack));
    }
}

#[inline]
pub unsafe fn wrgsbase(base: VirtAddr) {
    // TODO: consider using the `wrgsbase` instruction when available
    unsafe {
        wrmsr(IA32_GS_BASE, base.as_u64());
    }
}

#[inline]
pub unsafe fn read_gs_qword<const OFF: usize>() -> u64 {
    let ret: u64;
    unsafe {
        asm!("mov {}, gs:[{}]", out(reg) ret, const OFF);
    }
    ret
}
