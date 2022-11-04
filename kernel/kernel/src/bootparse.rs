use core::slice;

use bootinfo::item::{FramebufferInfo, MemoryRange};
use bootinfo::view::View;
use bootinfo::ItemKind;

use crate::mm::physmap::paddr_to_physmap;
use crate::mm::types::PhysAddr;

/// Encapsulates data from a parsed bootinfo view created by the loader.
pub struct BootinfoData {
    memory_map: &'static [MemoryRange],
    efi_system_table: Option<PhysAddr>,
    framebuffer_info: Option<&'static FramebufferInfo>,
}

impl BootinfoData {
    /// Parses the physical memory range `paddr..paddr + size` as a bootinfo structure and returns
    /// a parsed view representing it.
    ///
    /// # Safety
    ///
    /// * The specified physical range must contain a valid bootinfo structure
    /// * The physmap must be initialized and cover the specified range
    /// * The caller must guarantee that the physical memory range will remian valid and not be
    ///   repurposed for the duration of the lifetime of the returned object
    pub unsafe fn parse(paddr: PhysAddr, size: usize) -> Self {
        let mut memory_map = None;
        let mut efi_system_table = None;
        let mut framebuffer_info: Option<&FramebufferInfo> = None;

        // Safety: function contract
        let buffer = unsafe { slice::from_raw_parts(paddr_to_physmap(paddr).as_ptr(), size) };
        let view = View::new(buffer).expect("invalid bootinfo");

        for item in view.items() {
            match item.kind() {
                ItemKind::MEMORY_MAP => {
                    memory_map =
                        Some(unsafe { item.get_slice() }.expect("invalid bootinfo memory map"));
                }
                ItemKind::EFI_SYSTEM_TABLE => {
                    efi_system_table =
                        Some(unsafe { item.read() }.expect("invalid bootinfo EFI system table"));
                }
                ItemKind::FRAMEBUFFER => {
                    framebuffer_info =
                        Some(unsafe { item.get() }.expect("invalid bootinfo framebuffer"));
                }
                _ => {}
            }
        }

        Self {
            memory_map: memory_map.expect("no memory map in bootinfo"),
            efi_system_table,
            framebuffer_info,
        }
    }

    /// Returns the memory map provided in the bootinfo.
    pub fn memory_map(&self) -> &[MemoryRange] {
        self.memory_map
    }

    /// Returns the physical address of the EFI system table provided in the bootinfo, if present.
    pub fn efi_system_table(&self) -> Option<PhysAddr> {
        self.efi_system_table
    }

    /// Returns the framebuffer information provided in the bootinfo, if present.
    pub fn framebuffer_info(&self) -> Option<&FramebufferInfo> {
        self.framebuffer_info
    }
}
