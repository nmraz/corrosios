use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::types::Guid;

pub mod fs;
pub mod image;
pub mod io;
pub mod path;

/// # Safety
///
/// The reported GUID value must be correct, as it will be trusted by unsafe code casting to the
/// correct protocol pointer.
pub unsafe trait Protocol {
    type Abi;
    const GUID: Guid;

    /// # Safety
    ///
    /// ABI pointer must be valid and outlive the protocol instance.
    unsafe fn from_abi(abi: *mut Self::Abi) -> Self;

    fn abi(&self) -> *mut Self::Abi;
}

pub struct ProtocolHandle<'a, P>(P, PhantomData<&'a ()>);

impl<P: Protocol> ProtocolHandle<'_, P> {
    /// # Safety
    ///
    /// ABI pointer must be valid and outlive `'a`.
    pub(crate) unsafe fn from_abi(abi: *mut P::Abi) -> Self {
        // Safety: function preconditions.
        Self(unsafe { P::from_abi(abi) }, PhantomData)
    }
}

impl<P> Deref for ProtocolHandle<'_, P> {
    type Target = P;

    fn deref(&self) -> &P {
        &self.0
    }
}

impl<P> DerefMut for ProtocolHandle<'_, P> {
    fn deref_mut(&mut self) -> &mut P {
        &mut self.0
    }
}

macro_rules! unsafe_protocol {
    ($name:ident($abi:ty, $guid:literal);) => {
        pub struct $name(*mut $abi);

        unsafe impl crate::proto::Protocol for $name {
            type Abi = $abi;

            const GUID: crate::types::Guid = crate::guid!($guid);

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
            let abi = $p.abi();
            ((*abi).$name)(abi, $($args),*)
        }
    };
}

// Hoist definition
use abi_call;
