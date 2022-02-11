use crate::mm::types::VirtFrame;

use super::PT_LEVEL_SHIFT;

pub fn pt_index(frame: VirtFrame, level: usize) -> usize {
    frame.as_usize() >> (PT_LEVEL_SHIFT * (level - 1))
}
