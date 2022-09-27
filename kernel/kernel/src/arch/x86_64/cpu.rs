use super::x64_cpu::{cli, get_rflags, hlt, sti};

#[inline]
pub fn halt() -> ! {
    unsafe {
        cli();
    }
    loop {
        hlt();
    }
}

pub fn irq_enabled() -> bool {
    get_rflags() & 0x200 != 0
}

#[inline]
pub unsafe fn disable_irq() {
    unsafe {
        cli();
    }
}

#[inline]
pub unsafe fn enable_irq() {
    unsafe {
        sti();
    }
}
