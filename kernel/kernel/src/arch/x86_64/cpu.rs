use core::arch::asm;

use super::x64_cpu::get_rflags;

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
