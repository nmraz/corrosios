use core::panic::PanicInfo;

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {}
}
