use log::debug;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END};
use crate::err::{Error, Result};

use super::types::{AccessMode, AccessType, VirtAddr};

pub mod aspace;
pub mod kernel_aspace;
pub mod low_aspace;
pub mod object;

/// Initializes the VM subsystem, including the global kernel address space.
pub fn init() {
    debug!("initializing VM system");
    kernel_aspace::init();
}

/// Handles a page fault that occurred while accessing `addr` with the specified access type and
/// mode.
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
