//! The RomFS header and its directory and file entry structures.
//!
//! RomFS stores its tree as four tables the header locates: a hash table and a
//! metadata table for directories, and the same pair for files. Entries live in
//! the metadata tables and are chained by offsets into them, so an entry is
//! addressed by its byte offset rather than by an index.
//!
//! Each entry is followed by its variable-length name, which is why the asserted
//! sizes here cover only the fixed prefix.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{U32, U64},
};

/// Locates the four tables the tree is stored in, and the region the file contents sit in.
///
/// Every offset is from the start of the image. The hash tables turn a name lookup into a bucket
/// probe, and the metadata tables hold the entries themselves; a lookup reads the hash table to
/// find a candidate entry, then walks the chain in the metadata table.
///
/// See <https://switchbrew.org/wiki/RomFS#Header>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsHeader {
    /// Length of this header in bytes, always `0x50`.
    pub header_size: U64,
    /// Offset of the directory hash table, from the start of the image.
    pub dir_hash_table_offset: U64,
    /// Length of the directory hash table in bytes, a whole number of `u32` buckets.
    pub dir_hash_table_size: U64,
    /// Offset of the directory metadata table, from the start of the image.
    pub dir_meta_table_offset: U64,
    /// Length of the directory metadata table in bytes.
    pub dir_meta_table_size: U64,
    /// Offset of the file hash table, from the start of the image.
    pub file_hash_table_offset: U64,
    /// Length of the file hash table in bytes, a whole number of `u32` buckets.
    pub file_hash_table_size: U64,
    /// Offset of the file metadata table, from the start of the image.
    pub file_meta_table_offset: U64,
    /// Length of the file metadata table in bytes.
    pub file_meta_table_size: U64,
    /// Offset of the file data region, from the start of the image.
    ///
    /// Every [`RomFsFileEntry::data_offset`] is measured from here rather than from the image,
    /// so the region can move without rewriting a single entry.
    pub file_data_offset: U64,
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#Header
const_assert_eq!(size_of::<RomFsHeader>(), 0x50);
const_assert_eq!(align_of::<RomFsHeader>(), 0x1);

/// One directory in the tree, as stored in the directory metadata table.
///
/// The tree is threaded rather than nested: a directory names its first child and its next sibling,
/// and walking a directory means following `child_offset` once and then `sibling_offset` until the
/// chain ends. Every offset is a byte offset into the directory metadata table, and `U32::MAX`
/// terminates a chain.
///
/// The `0x18` bytes asserted below are the fixed prefix only; the entry's name follows it inline.
///
/// See <https://switchbrew.org/wiki/RomFS#Directory_Entry>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsDirEntry {
    /// The directory this one sits in, or `U32::MAX` for the root, which is its own parent.
    pub parent_offset: U32,
    /// The next directory sharing this parent, or `U32::MAX` at the end of the chain.
    pub sibling_offset: U32,
    /// The first subdirectory, or `U32::MAX` when there are none.
    pub child_offset: U32,
    /// The first file in this directory, or `U32::MAX` when there are none.
    pub file_offset: U32,
    /// The next entry in the same hash bucket, or `U32::MAX` at the end of the chain.
    pub hash_sibling_offset: U32,
    /// Length of the name following this entry, in bytes.
    pub name_len: U32,
    // The UTF-8 name follows inline, padded to a 4-byte boundary.
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#Directory_Entry
const_assert_eq!(size_of::<RomFsDirEntry>(), 0x18);
const_assert_eq!(align_of::<RomFsDirEntry>(), 0x1);

/// One file in the tree, as stored in the file metadata table.
///
/// Files chain the same way directories do: a directory names its first file, and each file names
/// the next one beside it. The entry carries the file's bounds rather than its bytes, which live in
/// the data region.
///
/// The `0x20` bytes asserted below are the fixed prefix only; the entry's name follows it inline.
///
/// See <https://switchbrew.org/wiki/RomFS#File_Entry>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsFileEntry {
    /// The directory this file sits in, as an offset into the directory metadata table.
    pub parent_offset: U32,
    /// The next file sharing this parent, or `U32::MAX` at the end of the chain.
    pub sibling_offset: U32,
    /// Offset of the file's first byte, from [`RomFsHeader::file_data_offset`].
    pub data_offset: U64,
    /// Length of the file in bytes.
    pub data_size: U64,
    /// The next entry in the same hash bucket, or `U32::MAX` at the end of the chain.
    pub hash_sibling_offset: U32,
    /// Length of the name following this entry, in bytes.
    pub name_len: U32,
    // The UTF-8 name follows inline, padded to a 4-byte boundary.
}

// Verify struct size - https://switchbrew.org/wiki/RomFS#File_Entry
const_assert_eq!(size_of::<RomFsFileEntry>(), 0x20);
const_assert_eq!(align_of::<RomFsFileEntry>(), 0x1);
