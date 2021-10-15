pub const PAGE_SIZE: usize = 0x1000;

pub const fn to_page_count(bytes: usize) -> usize {
    (bytes + PAGE_SIZE - 1) / PAGE_SIZE
}
