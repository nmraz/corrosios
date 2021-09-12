use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::types::Guid;

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

pub struct ProtocolHandle<'a, P: Protocol>(P, PhantomData<&'a ()>);

impl<'a, P: Protocol> ProtocolHandle<'a, P> {
    /// # Safety
    ///
    /// ABI pointer must be valid and outlive `'a`.
    pub(crate) unsafe fn from_abi(abi: *mut P::Abi) -> Self {
        Self(P::from_abi(abi), PhantomData)
    }
}

impl<'a, P: Protocol> Deref for ProtocolHandle<'a, P> {
    type Target = P;

    fn deref(&self) -> &P {
        &self.0
    }
}

impl<'a, P: Protocol> DerefMut for ProtocolHandle<'a, P> {
    fn deref_mut(&mut self) -> &mut P {
        &mut self.0
    }
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
