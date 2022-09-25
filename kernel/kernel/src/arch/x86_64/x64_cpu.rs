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
