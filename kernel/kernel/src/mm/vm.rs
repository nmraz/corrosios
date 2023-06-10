use log::debug;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END, LOW_ASPACE_END};
use crate::err::{Error, Result};
use crate::sync::resched::ReschedGuard;

use super::types::{AccessMode, AccessType, VirtAddr};

pub mod aspace;
pub mod kernel_aspace;
pub mod low_aspace;
pub mod object;

pub struct PerCpu {
    aspace_context: low_aspace::Context,
}

impl PerCpu {
    /// Creates a new per-cpu VM context.
    ///
    /// Note: this function may be called very early during initialization (before anything is set
    /// up), so it must not allocate or take any locks.
    pub fn new() -> Self {
        Self {
            aspace_context: low_aspace::Context::new(),
        }
    }
}

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
    } else if is_low_addr(addr) {
        // Note: we snapshot the current low address space in preparation for the fact that the
        // handler will later run with preemption enabled.
        let current_low_aspace =
            low_aspace::current(&ReschedGuard::new()).ok_or(Error::BAD_ADDRESS)?;
        current_low_aspace.fault(addr.containing_page(), access_type)
    } else {
        Err(Error::BAD_ADDRESS)
    }
}

fn is_low_addr(addr: VirtAddr) -> bool {
    addr.containing_page() < LOW_ASPACE_END
}

fn is_kernel_addr(addr: VirtAddr) -> bool {
    (KERNEL_ASPACE_BASE..KERNEL_ASPACE_END).contains(&addr.containing_page())
}
