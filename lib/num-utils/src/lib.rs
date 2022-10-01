#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

pub const fn align_down(val: usize, align: usize) -> usize {
    (val / align) * align
}

pub const fn align_up(val: usize, align: usize) -> usize {
    align_down(val + align - 1, align)
}

pub const fn div_ceil(val: usize, divisor: usize) -> usize {
    (val + divisor - 1) / divisor
}

pub const fn log2(val: usize) -> usize {
    (usize::BITS - val.leading_zeros() - 1) as usize
}

pub const fn log2_ceil(val: usize) -> usize {
    if val <= 1 {
        return 0;
    }

    log2(val - 1) + 1
}
