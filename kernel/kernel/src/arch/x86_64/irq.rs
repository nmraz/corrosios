pub fn idle_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
