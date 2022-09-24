use core::arch::asm;

#[inline]
pub fn halt() -> ! {
    unsafe {
        asm!("cli", options(nomem, nostack));
        loop {
            asm!("hlt", options(nomem, nostack));
        }
    }
}

pub fn irq_enabled() -> bool {
    get_rflags() & 0x200 != 0
}

#[inline]
pub unsafe fn disable_irq() {
    unsafe {
        asm!("cli", options(nostack));
    }
}

#[inline]
pub unsafe fn enable_irq() {
    unsafe {
        asm!("sti", options(nostack));
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
