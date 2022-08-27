use core::ops::Range;

use bootinfo::item::{MemoryKind, MemoryRange};

use super::types::PhysFrameNum;

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

/// Invoke `func` for every memory range reported as usable in `mem_map`, carving out holes for
/// any ranges in `reserved_ranges`.
///
/// **Note**: This function assumes that both `mem_map` and `reserved_ranges` are sorted in
/// ascending order, and that the ranges contained in each are disjoint.
pub fn iter_usable_ranges(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
    mut func: impl FnMut(PhysFrameNum, PhysFrameNum),
) {
    let mut reserved_ranges = reserved_ranges.iter().peekable();

    'outer: for Range { mut start, end } in usable_ranges(mem_map) {
        // Chop up our usable range based on the reserved ranges that intersect it. This loop should
        // always consume all reserved ranges contained in `[0, end)`.
        while let Some(reserved) = reserved_ranges.peek().copied() {
            assert!(reserved.start <= reserved.end);

            if reserved.start >= end || reserved.end < start {
                // The next reserved range doesn't intersect us at all, so we're done here; just
                // make sure to report the remaining usable range below.
                break;
            }

            // Beyond this point: `reserved.start < end && reserved.end >= start`.

            if reserved.start > start {
                // We have a gap before the reserved range, report it.
                func(start, reserved.start);
            }
            start = reserved.end;

            if start <= end {
                // We're done with this reserved range now.
                reserved_ranges.next();
            }

            if start >= end {
                // We've covered all of the original usable range, try the next one.
                continue 'outer;
            }
        }

        // Deal with the tail/non-intersecting portion of the range.
        if start < end {
            func(start, end);
        }
    }
}

pub fn usable_ranges(
    mem_map: &[MemoryRange],
) -> impl DoubleEndedIterator<Item = Range<PhysFrameNum>> + '_ {
    mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE)
        .map(|range| {
            let start = PhysFrameNum::new(range.start_page);
            start..start + range.page_count
        })
}
