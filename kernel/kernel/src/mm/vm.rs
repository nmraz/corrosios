use log::debug;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END};
use crate::err::{Error, Result};

use super::types::{AccessMode, AccessType, VirtAddr};

pub mod aspace;
pub mod object;

mod kernel_aspace;

pub fn init() {
    debug!("initializing VM system");
    kernel_aspace::init();
}

pub fn page_fault(addr: VirtAddr, access_type: AccessType, access_mode: AccessMode) -> Result<()> {
    if access_mode == AccessMode::Kernel && is_kernel_addr(addr) {
        kernel_aspace::get().fault(addr.containing_page(), access_type)
    } else {
        Err(Error::BAD_ADDRESS)
    }
}

fn is_kernel_addr(addr: VirtAddr) -> bool {
    (KERNEL_ASPACE_BASE..KERNEL_ASPACE_END).contains(&addr.containing_page())
}
