use crate::types::Guid;

pub mod image;
pub mod io;

pub unsafe trait Protocol {
    type Abi;
    const GUID: Guid;

    /// # Safety
    ///
    /// ABI pointer must be valid and outlive the protocol instance.
    unsafe fn from_abi(abi: *mut Self::Abi) -> Self;

    fn abi(&self) -> *mut Self::Abi;
}

macro_rules! unsafe_protocol {
    ($name:ident($abi:ty, $guid:tt);) => {
        pub struct $name(*mut $abi);

        unsafe impl crate::proto::Protocol for $name {
            type Abi = $abi;

            const GUID: crate::types::Guid = crate::types::Guid $guid;

            unsafe fn from_abi(abi: *mut Self::Abi) -> Self {
                Self(abi)
            }

            fn abi(&self) -> *mut Self::Abi {
                self.0
            }
        }
    };
}

// Hoist definition
use unsafe_protocol;

macro_rules! abi_call {
    ($p:ident, $name:ident($($args:expr),*)) => {
        {
            use crate::proto::Protocol;

            let abi = $p.abi();
            ((*abi).$name)(abi, $($args),*)
        }
    };
}

// Hoist definition
use abi_call;
