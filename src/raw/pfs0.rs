use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// PFS0 magic number: "PFS0" in ASCII (0x30534650).
pub const PFS0_MAGIC: u32 = 0x30534650;

/// PFS0 header (0x10 bytes) describing the filesystem structure.
///
/// Contains file count, string table size, and reserved fields.
/// The header is followed by file entry table, string table, and file data.
///
/// See: <https://switchbrew.org/wiki/PFS0>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Pfs0Header {
    /// Magic number (must be [`PFS0_MAGIC`])
    pub magic: U32,
    /// Number of file entries
    pub file_count: U32,
    /// Size of string table in bytes
    pub string_table_size: U32,
    /// Reserved (zero)
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/PFS0#Header
const_assert_eq!(size_of::<Pfs0Header>(), 0x10);
const_assert_eq!(align_of::<Pfs0Header>(), 0x1);

/// PFS0 file entry (0x18 bytes).
///
/// File entries are stored in a table immediately following the PFS0 header.
/// Each entry describes a file's location, size, and name location in the string table.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Pfs0FileEntry {
    /// Offset to file data (relative to file data region start)
    pub offset: U64,
    /// Size of file data in bytes
    pub size: U64,
    /// Offset to filename in string table
    pub string_table_offset: U32,
    /// Reserved (normally zero)
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/PFS0#File_Entry
const_assert_eq!(size_of::<Pfs0FileEntry>(), 0x18);
const_assert_eq!(align_of::<Pfs0FileEntry>(), 0x1);
