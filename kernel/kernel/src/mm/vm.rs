use log::debug;

use crate::arch::mm::LOW_ASPACE_END;
use crate::err::{Error, Result};
use crate::sched::Thread;

use super::types::{AccessType, VirtAddr};

pub use self::kernel_aspace::get as get_kernel_addr_space;
pub use self::low_aspace::{make_low_addr_space, switch_to as switch_low_addr_space, LowAddrSpace};

pub mod aspace;
pub mod object;

mod kernel_aspace;
mod low_aspace;

/// Initializes the VM subsystem, including the global kernel address space.
pub fn init() {
    debug!("initializing VM system");
    kernel_aspace::init();
}

/// Handles a page fault that occurred while accessing `addr` with the specified access type and
/// mode.
pub fn page_fault(addr: VirtAddr, access_type: AccessType) -> Result<()> {
    if is_low_addr(addr) {
        let current_thread = Thread::current().ok_or(Error::INVALID_STATE)?;
        let aspace = current_thread.addr_space().ok_or(Error::BAD_ADDRESS)?;
        aspace.fault(addr.containing_page(), access_type)
    } else {
        Err(Error::BAD_ADDRESS)
    }
}

fn is_low_addr(addr: VirtAddr) -> bool {
    addr.containing_page() < LOW_ASPACE_END
}
