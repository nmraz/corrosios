use core::fmt;
use core::ops::Range;

use bootinfo::item::MemoryKind;

use super::types::PhysFrameNum;

pub fn is_usable(kind: MemoryKind) -> bool {
    // Note: we include boot services here as they can be reclaimed once we are done parsing
    // data provided by the firmware.
    matches!(kind, MemoryKind::USABLE | MemoryKind::FIRMWARE_BOOT)
}

pub fn is_early_usable(kind: MemoryKind) -> bool {
    // Note: we intentionally exclude boot services here, as we may still need to access data stored
    // in that kind of memory and will explicitly reclaim it later.
    kind == MemoryKind::USABLE
}

/// Invokes `func` for every memory range reported as usable in `usable_ranges`, carving out holes
/// for any ranges in `reserved_ranges`.
///
/// **Note**: This function assumes that both `usable_ranges` and `reserved_ranges` are sorted in
/// ascending order, and that the ranges contained in each are disjoint.
pub fn iter_usable_ranges(
    usable_ranges: impl Iterator<Item = Range<PhysFrameNum>>,
    reserved_ranges: &[Range<PhysFrameNum>],
    mut func: impl FnMut(PhysFrameNum, PhysFrameNum),
) {
    let mut reserved_ranges = reserved_ranges.iter().peekable();

    'outer: for Range { mut start, end } in usable_ranges {
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

pub fn display_byte_size(bytes: usize) -> impl fmt::Display {
    struct DisplayByteSize(usize);
    impl fmt::Display for DisplayByteSize {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.0 < 1024 {
                write!(f, "{}B", self.0)
            } else if self.0 < 1024 * 1024 {
                write!(f, "{}K", self.0 / 1024)
            } else if self.0 < 1024 * 1024 * 1024 {
                write!(f, "{}M", self.0 / (1024 * 1024))
            } else {
                write!(f, "{}G", self.0 / (1024 * 1024 * 1024))
            }
        }
    }

    DisplayByteSize(bytes)
}
