use core::str::{self, Utf8Chunks};
use core::{fmt, slice};

use bootinfo::item::{FramebufferInfo, MemoryRange};
use bootinfo::view::View;
use bootinfo::ItemKind;
use itertools::Itertools;

use crate::mm::physmap::paddr_to_physmap;
use crate::mm::types::PhysAddr;

/// A parsed command-line argument, with its name and value.
#[derive(Clone, Copy)]
pub struct CommandLineArg<'a> {
    /// The name of the argument.
    pub name: &'a [u8],
    /// The value of the argument, provided after the name.
    pub value: &'a [u8],
}

impl<'a> CommandLineArg<'a> {
    /// Parses a `name=value` type of argument out of `buf`.
    ///
    /// If the value is missing, it is returned as an empty slice.
    pub fn parse(buf: &'a [u8]) -> Self {
        let val_delim_pos = buf.iter().position(|&b| b == b'=');

        let (name, value) = if let Some(val_delim_pos) = val_delim_pos {
            (&buf[..val_delim_pos], &buf[val_delim_pos + 1..])
        } else {
            (buf, &b""[..])
        };

        Self { name, value }
    }
}

impl fmt::Display for CommandLineArg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_utf8_lossy(f, self.name)?;
        write!(f, "=")?;
        display_utf8_lossy(f, self.value)
    }
}

/// A parsed kernel command line, containing all arguments with their values.
#[derive(Clone, Copy)]
pub struct CommandLine<'a>(&'a [u8]);

impl<'a> CommandLine<'a> {
    /// Creates a new command line with the contents of `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self(buf)
    }

    /// Returns an iterator over all arguments in this command line.
    pub fn args(&self) -> impl DoubleEndedIterator<Item = CommandLineArg<'a>> {
        let items = self
            .0
            .split(u8::is_ascii_whitespace)
            .filter(|s| !s.is_empty());

        items.map(CommandLineArg::parse)
    }

    /// Retrives the value of the argument `name`, if present, or returns `None` if not.
    ///
    /// Note that this function will return `Some("")` if the argument is present but has no value.
    pub fn get_arg_value(&self, name: &str) -> Option<&'a [u8]> {
        let name = name.as_bytes();
        self.args()
            .rfind(|arg| arg.name == name)
            .map(|arg| arg.value)
    }

    /// Attempts to retrieve the value of the argument `name` as a UTF-8 string.
    ///
    /// If the argument is not present or contains invalid UTF-8, `None` will be returned.
    ///
    /// Note that this function will return `Some("")` if the argument is present but has no value.
    pub fn get_arg_str_value(&self, name: &str) -> Option<&'a str> {
        self.get_arg_value(name)
            .and_then(|val| str::from_utf8(val).ok())
    }
}

impl fmt::Display for CommandLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.args().format(" "))
    }
}

/// Encapsulates data from a parsed bootinfo view created by the loader.
pub struct BootinfoData<'a> {
    memory_map: &'a [MemoryRange],
    efi_system_table: Option<PhysAddr>,
    framebuffer_info: Option<&'a FramebufferInfo>,
    command_line: CommandLine<'a>,
}

impl<'a> BootinfoData<'a> {
    /// Parses the physical memory range `paddr..paddr + size` as a bootinfo structure and returns
    /// a parsed view representing it.
    ///
    /// # Safety
    ///
    /// * The specified physical range must contain a valid bootinfo structure
    /// * The physmap must be initialized and cover the specified range
    /// * The caller must guarantee that the physical memory range will remian valid and not be
    ///   repurposed for the duration of the lifetime of the returned object
    pub unsafe fn parse_phys(paddr: PhysAddr, size: usize) -> Self {
        let buffer = unsafe { slice::from_raw_parts(paddr_to_physmap(paddr).as_ptr(), size) };
        Self::parse(buffer)
    }

    /// Parses the data in `buffer` as a bootinfo structure and returns a parsed view representing
    /// it.
    pub fn parse(buffer: &'a [u8]) -> Self {
        let mut memory_map = None;
        let mut efi_system_table = None;
        let mut framebuffer_info = None;
        let mut command_line = None;

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
                ItemKind::COMMAND_LINE => {
                    command_line =
                        Some(unsafe { item.get_slice() }.expect("invalid bootinfo command line"));
                }
                _ => {}
            }
        }

        Self {
            memory_map: memory_map.expect("no memory map in bootinfo"),
            efi_system_table,
            framebuffer_info,
            command_line: CommandLine::new(command_line.unwrap_or(b"")),
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

    /// Returns the kernel command line provided in the bootinfo.
    pub fn command_line(&self) -> CommandLine<'_> {
        self.command_line
    }
}

fn display_utf8_lossy(f: &mut fmt::Formatter<'_>, buf: &[u8]) -> fmt::Result {
    for chunk in Utf8Chunks::new(buf) {
        write!(f, "{}", chunk.valid())?;
        if !chunk.invalid().is_empty() {
            write!(f, "{}", char::REPLACEMENT_CHARACTER)?;
        }
    }

    Ok(())
}
