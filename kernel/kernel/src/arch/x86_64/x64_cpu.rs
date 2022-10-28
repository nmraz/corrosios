use core::arch::asm;

use bitflags::bitflags;

use crate::mm::types::VirtAddr;

const IA32_PAT: u32 = 0x277;
const IA32_GS_BASE: u32 = 0xc0000101;
const IA32_EFER: u32 = 0xc0000080;

bitflags! {
    #[repr(transparent)]
    pub struct Rflags: u64 {
        const CF = 1 << 0;
        const RSVD = 1 << 1;
        const PF = 1 << 2;
        const AF = 1 << 4;
        const ZF = 1 << 6;
        const SF = 1 << 7;
        const TF = 1 << 8;
        const IF = 1 << 9;
        const DF = 1 << 10;
        const OF = 1 << 11;
        const IOPL3 = 3 << 12;
        const NT = 1 << 14;
        const RF = 1 << 16;
        const VM = 1 << 17;
        const AC = 1 << 18;
        const VIF = 1 << 19;
        const VIP = 1 << 20;
        const ID = 1 << 21;
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct Cr0: u64 {
        /// Protection enable
        const PE = 1 << 0;

        /// Monitor coprocessor
        const MP = 1 << 1;

        /// FPU emulation
        const EM = 1 << 2;

        /// Hardware task switching
        const TS = 1 << 3;

        /// Extension type (always 1)
        const ET = 1 << 4;

        /// Numeric error (controls x87 exceptions)
        const NE = 1 << 5;

        /// Supervisor-level write protection
        const WP = 1 << 16;

        /// Alignment mask
        const AM = 1 << 18;

        /// Not write-through
        const NW = 1 << 29;

        /// Cache disable
        const CD = 1 << 30;

        /// Paging enabled
        const PG = 1 << 31;
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct Cr4: u64 {
        const VME = 1 << 0;
        const PVI = 1 << 1;
        const TSD = 1 << 2;
        const DE = 1 << 3;
        const PSE = 1 << 4;
        const PAE = 1 << 5;
        const MCE = 1 << 6;
        const PGE = 1 << 7;
        const PCE = 1 << 8;
        const OSFXCR = 1 << 9;
        const OSXMMEXCPT = 1 << 10;
        const UMIP = 1 << 11;
        const FSGSBASE = 1 << 16;
        const PCIDE = 1 << 17;
        const OSXSAVE = 1 << 18;
        const SMEP = 1 << 20;
        const SMAP = 1 << 21;
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct Ia32Efer: u64 {
        /// Syscall enable
        const SCE = 1 << 0;

        /// Long mode enable
        const LME = 1 << 8;

        /// Long mode active
        const LMA = 1 << 10;

        /// NX bit enable
        const NXE = 1 << 11;
    }
}

#[repr(C, packed(2))]
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
pub unsafe fn lgdt(desc: &DescriptorRegister) {
    unsafe {
        asm!("lgdt [{}]", in(reg) desc, options(nostack));
    }
}

#[inline]
pub unsafe fn lldt(selector: u16) {
    unsafe {
        asm!("lldt {:x}", in(reg) selector, options(nostack));
    }
}

#[inline]
pub unsafe fn ltr(tss_selector: u16) {
    unsafe {
        asm!("ltr {:x}", in(reg) tss_selector, options(nostack));
    }
}

#[inline]
pub unsafe fn lidt(desc: &DescriptorRegister) {
    unsafe {
        asm!("lidt [{}]", in(reg) desc, options(nostack));
    }
}

#[inline]
pub fn get_rflags() -> Rflags {
    let rflags: u64;
    unsafe {
        asm!("pushf; pop {}", out(reg) rflags);
        Rflags::from_bits_unchecked(rflags)
    }
}

#[inline]
pub fn read_cr0() -> Cr0 {
    let cr0: u64;
    unsafe {
        asm!("mov {}, cr0", out(reg) cr0, options(nostack));
        Cr0::from_bits_unchecked(cr0)
    }
}

#[inline]
pub unsafe fn write_cr0(cr0: Cr0) {
    let cr0 = cr0.bits();
    unsafe {
        asm!("mov cr0, {}", in(reg) cr0, options(nostack));
    }
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
pub fn read_cr4() -> Cr4 {
    let cr4: u64;
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nostack));
        Cr4::from_bits_unchecked(cr4)
    }
}

#[inline]
pub unsafe fn write_cr4(cr4: Cr4) {
    let cr4 = cr4.bits();
    unsafe {
        asm!("mov cr4, {}", in(reg) cr4, options(nostack));
    }
}

#[inline]
pub fn read_cr2() -> VirtAddr {
    let cr2: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) cr2, options(nostack));
    }
    VirtAddr::new(cr2 as usize)
}

#[inline]
pub fn read_ia32_efer() -> Ia32Efer {
    unsafe { Ia32Efer::from_bits_unchecked(rdmsr(IA32_EFER)) }
}

#[inline]
pub unsafe fn write_ia32_efer(ia32_efer: Ia32Efer) {
    unsafe {
        wrmsr(IA32_EFER, ia32_efer.bits());
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
        asm!("mov {}, gs:[{}]", out(reg) ret, const OFF, options(nostack, readonly, pure));
    }
    ret
}

#[inline]
unsafe fn rdmsr(num: u32) -> u64 {
    let eax: u32;
    let edx: u32;

    unsafe {
        asm!("rdmsr", in("ecx") num, out("eax") eax, out("edx") edx, options(nostack));
    }

    ((edx as u64) << 32) | (eax as u64)
}

#[inline]
unsafe fn wrmsr(num: u32, val: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") num, in("eax") val as u32, in("edx") (val >> 32) as u32, options(nostack));
    }
}
