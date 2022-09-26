use core::arch::asm;

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
pub fn get_rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!("pushf; pop {}", out(reg) rflags);
    }
    rflags
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
