#![no_std]

use core::borrow::{Borrow, BorrowMut};

use num_utils::div_ceil;

pub const fn bytes_required(size: usize) -> usize {
    div_ceil(size, 8)
}

pub type BorrowedBitmap<'a> = Bitmap<&'a [u8]>;
pub type BorrowedBitmapMut<'a> = Bitmap<&'a mut [u8]>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bitmap<B> {
    bytes: B,
}

impl<B: Borrow<[u8]>> Bitmap<B> {
    pub fn new(bytes: B) -> Self {
        Self { bytes }
    }

    pub fn get(&self, index: usize) -> bool {
        let (byte, bit) = split_index(index);
        ((self.bytes()[byte] >> bit) & 1) != 0
    }

    pub fn first_zero(&self, limit: usize) -> Option<usize> {
        // TODO: optimize this
        (0..limit).find(|&index| !self.get(index))
    }

    fn bytes(&self) -> &[u8] {
        self.bytes.borrow()
    }
}

impl<B: BorrowMut<[u8]>> Bitmap<B> {
    pub fn set(&mut self, index: usize) {
        let (byte, bit) = split_index(index);
        self.bytes_mut()[byte] |= 1 << bit;
    }

    pub fn unset(&mut self, index: usize) {
        let (byte, bit) = split_index(index);
        self.bytes_mut()[byte] &= !(1u8 << bit);
    }

    pub fn toggle(&mut self, index: usize) {
        let (byte, bit) = split_index(index);
        self.bytes_mut()[byte] ^= 1 << bit;
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.borrow_mut()
    }
}

fn split_index(index: usize) -> (usize, usize) {
    (index / 8, index % 8)
}
