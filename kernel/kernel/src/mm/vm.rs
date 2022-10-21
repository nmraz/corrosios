use log::debug;

pub mod aspace;
pub mod object;

mod kernel_aspace;

pub fn init() {
    debug!("initializing VM system");
    kernel_aspace::init();
}
