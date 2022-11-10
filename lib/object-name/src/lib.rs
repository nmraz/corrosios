//! A small library for inline strings containing names for debugging purposes.

#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use core::borrow::Borrow;
use core::{cmp, fmt};

use arrayvec::ArrayString;

const MAX_NAME_LEN: usize = 32;

/// An inline, fixed length string intended for storing the names of objects for debugging purposes.
///
/// The contents of this string may be truncated if it exceeds some implementation-defined limit,
/// and should not be relied upon for correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(ArrayString<MAX_NAME_LEN>);

impl Name {
    /// Creates a new name initialized with `name`.
    ///
    /// The name may be truncated if too long.
    pub fn new(name: &str) -> Self {
        Self(ArrayString::from(&name[..cmp::min(name.len(), MAX_NAME_LEN)]).unwrap())
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}
