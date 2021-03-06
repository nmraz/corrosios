use core::arch::asm;

pub unsafe fn inb(port: u16) -> u8 {
    let retval: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") retval);
    }
    retval
}

pub unsafe fn outb(port: u16, val: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") val);
    }
}
