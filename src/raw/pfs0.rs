//! The PFS0 header and file entry table.
//!
//! A PFS0 is four regions in a fixed order: the header, one entry per file, a
//! string table holding the names, and the file data. Neither of the two offsets
//! in an entry is absolute — `offset` is measured from the start of the data
//! region and `string_table_offset` from the start of the string table — so both
//! need the header's counts to resolve.
//!
//! Names in the string table are null-terminated and the entry carries no length
//! for them. The table is padded to a 0x20-byte boundary, so its recorded size
//! exceeds the sum of the names it holds.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{U32, U64},
};

/// Magic identifying a PFS0 header: `PFS0`, little-endian, at offset zero.
pub const PFS0_MAGIC: u32 = 0x30534650;

/// The counts that fix where every later region of the archive begins.
///
/// Nothing in a PFS0 records an absolute offset for its regions: the entry table starts right after
/// this header, and the string table and data region follow at distances derived from `file_count`
/// and `string_table_size`. Both fields are therefore load-bearing, and a wrong one shifts every
/// file in the archive.
///
/// See <https://switchbrew.org/wiki/NCA#PFS0>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Pfs0Header {
    /// Always [`PFS0_MAGIC`]; anything else means this is not a PFS0.
    pub magic: U32,
    /// Number of entries in the file table that follows this header.
    pub file_count: U32,
    /// Length of the string table in bytes, including the padding to a `0x20`-byte boundary.
    pub string_table_size: U32,
    /// Unused by the format; zero in every archive this crate writes.
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NCA#PFS0
const_assert_eq!(size_of::<Pfs0Header>(), 0x10);
const_assert_eq!(align_of::<Pfs0Header>(), 0x1);

/// One file in the archive: where its bytes are, how many there are, and where its name is.
///
/// Entries sit in a table immediately after [`Pfs0Header`], in the same order as the files in the
/// data region. Neither offset is absolute, and the two are measured from different origins.
///
/// See <https://switchbrew.org/wiki/NCA#PartitionEntry>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Pfs0FileEntry {
    /// Offset of the file's first byte, from the start of the data region.
    pub offset: U64,
    /// Length of the file in bytes.
    pub size: U64,
    /// Offset of the file's name, from the start of the string table.
    ///
    /// The name is NUL-terminated and its length is recorded nowhere, so reading it means
    /// scanning to the terminator.
    pub string_table_offset: U32,
    /// Unused by the format; zero in every archive this crate writes.
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NCA#PartitionEntry
const_assert_eq!(size_of::<Pfs0FileEntry>(), 0x18);
const_assert_eq!(align_of::<Pfs0FileEntry>(), 0x1);
