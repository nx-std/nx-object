use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// RomFS header (0x50 bytes) describing the filesystem structure.
///
/// Contains offsets and sizes for hash tables, metadata tables, and file data.
///
/// See: <https://switchbrew.org/wiki/RomFS>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsHeader {
    /// Size of this header (always 0x50)
    pub header_size: U64,
    /// Offset to directory hash table
    pub dir_hash_table_offset: U64,
    /// Size of directory hash table
    pub dir_hash_table_size: U64,
    /// Offset to directory metadata table
    pub dir_meta_table_offset: U64,
    /// Size of directory metadata table
    pub dir_meta_table_size: U64,
    /// Offset to file hash table
    pub file_hash_table_offset: U64,
    /// Size of file hash table
    pub file_hash_table_size: U64,
    /// Offset to file metadata table
    pub file_meta_table_offset: U64,
    /// Size of file metadata table
    pub file_meta_table_size: U64,
    /// Offset to file data region
    pub file_data_offset: U64,
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#Header
const_assert_eq!(size_of::<RomFsHeader>(), 0x50);
const_assert_eq!(align_of::<RomFsHeader>(), 0x1);

/// RomFS directory entry (0x18 bytes + variable-length name).
///
/// Directory entries are stored in the directory metadata table and
/// linked together via offsets to form the filesystem tree.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsDirEntry {
    /// Offset to parent directory (U32::MAX for root)
    pub parent_offset: U32,
    /// Offset to next sibling directory (U32::MAX if none)
    pub sibling_offset: U32,
    /// Offset to first child directory (U32::MAX if none)
    pub child_offset: U32,
    /// Offset to first file in this directory (U32::MAX if none)
    pub file_offset: U32,
    /// Offset to next entry in hash bucket (U32::MAX if end of chain)
    pub hash_sibling_offset: U32,
    /// Length of directory name in bytes
    pub name_len: U32,
    // Directory name follows (variable length UTF-8)
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#Directory_Entry
const_assert_eq!(size_of::<RomFsDirEntry>(), 0x18);
const_assert_eq!(align_of::<RomFsDirEntry>(), 0x1);

/// RomFS file entry (0x20 bytes + variable-length name).
///
/// File entries are stored in the file metadata table and
/// reference the actual file data via offset and size.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsFileEntry {
    /// Offset to parent directory
    pub parent_offset: U32,
    /// Offset to next sibling file (U32::MAX if none)
    pub sibling_offset: U32,
    /// Offset to file data (relative to file_data_offset)
    pub data_offset: U64,
    /// Size of file data in bytes
    pub data_size: U64,
    /// Offset to next entry in hash bucket (U32::MAX if end of chain)
    pub hash_sibling_offset: U32,
    /// Length of file name in bytes
    pub name_len: U32,
    // File name follows (variable length UTF-8)
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#File_Entry
const_assert_eq!(size_of::<RomFsFileEntry>(), 0x20);
const_assert_eq!(align_of::<RomFsFileEntry>(), 0x1);
