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
