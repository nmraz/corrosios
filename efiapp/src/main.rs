#![feature(lang_items, abi_efiapi)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "efiapi" fn efi_main() {}
