use bitflags::bitflags;

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const LEVEL_SHIFT: usize = 9;
pub const TABLE_ENTRIES: usize = 1 << LEVEL_SHIFT;

bitflags! {
    pub struct EntryFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_MODE = 1 << 2;

        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;

        const NO_EXEC = 1 << 63;
    }
}
