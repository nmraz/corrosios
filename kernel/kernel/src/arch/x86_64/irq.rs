use core::arch::asm;

pub fn idle_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
