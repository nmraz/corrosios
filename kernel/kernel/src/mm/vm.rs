use log::debug;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END, LOW_ASPACE_END};
use crate::err::{Error, Result};
use crate::sync::resched::ReschedGuard;

use super::types::{AccessType, VirtAddr};

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
pub fn page_fault(
    resched_guard: ReschedGuard,
    addr: VirtAddr,
    access_type: AccessType,
) -> Result<()> {
    if is_low_addr(addr) {
        // Snapshot the current (original) address space, and then enable rescheduling for the fault
        // itself.
        let current_low_aspace = low_aspace::current(&resched_guard).ok_or(Error::BAD_ADDRESS)?;
        drop(resched_guard);

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
