#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

#[doc(hidden)]
pub use core as _core;

#[macro_export]
macro_rules! struct_enum {
    (
        $(#[$attrs:meta])*
        $vis:vis struct $name:ident: $inner:ty {
            $($variants:ident = $vals:expr;)*
        }
    ) => {
        $(#[$attrs])*
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(transparent)]
        $vis struct $name($inner);

        impl $name {
            $(pub const $variants: Self = Self($vals);)*

            pub const fn to_raw(self) -> $inner {
                self.0
            }

            pub const fn from_raw(val: $inner) -> Self {
                Self(val)
            }
        }

        impl $crate::_core::fmt::Debug for $name {
            fn fmt(&self, f: &mut $crate::_core::fmt::Formatter<'_>) -> $crate::_core::fmt::Result {
                match *self {
                    $(Self::$variants => f.write_str(stringify!($variants)),)*
                    Self(val) => $crate::_core::write!(f, $crate::_core::concat!($crate::_core::stringify!($name), "({})"), val),
                }
            }
        }
    };
}
