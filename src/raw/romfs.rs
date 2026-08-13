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

/// The offset standing for "nothing here", which every chain in the tree ends with.
///
/// An entry is addressed by its byte offset into a metadata table, and zero is a valid one: it is
/// the root directory. So the format cannot spell absence as `0`, and reserves the largest offset
/// instead, which no table ever reaches. It terminates the sibling, child and hash chains alike,
/// and fills a hash bucket holding no entry.
pub const NO_ENTRY: u32 = 0xFFFF_FFFF;

/// Locates the four tables the tree is stored in, and the region the file contents sit in.
///
/// Every offset is from the start of the image. The hash tables turn a name lookup into a bucket
/// probe, and the metadata tables hold the entries themselves; a lookup reads the hash table to
/// find a candidate entry, then walks the chain in the metadata table.
///
/// See <https://www.3dbrew.org/wiki/RomFS#Level_3_Header_Format>.
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

// Verify struct size - https://www.3dbrew.org/wiki/RomFS#Level_3_Header_Format
const_assert_eq!(size_of::<RomFsHeader>(), 0x50);
const_assert_eq!(align_of::<RomFsHeader>(), 0x1);

/// One directory in the tree, as stored in the directory metadata table.
///
/// The tree is threaded rather than nested: a directory names its first child and its next sibling,
/// and walking a directory means following `child_offset` once and then `sibling_offset` until the
/// chain ends. Every offset is a byte offset into the directory metadata table, and [`NO_ENTRY`]
/// terminates a chain.
///
/// The `0x18` bytes asserted below are the fixed prefix only; the entry's name follows it inline.
///
/// See <https://www.3dbrew.org/wiki/RomFS#Directory_Metadata_Structure>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsDirEntry {
    /// The directory this one sits in, or [`NO_ENTRY`] for the root, which is its own parent.
    pub parent_offset: U32,
    /// The next directory sharing this parent, or [`NO_ENTRY`] at the end of the chain.
    pub sibling_offset: U32,
    /// The first subdirectory, or [`NO_ENTRY`] when there are none.
    pub child_offset: U32,
    /// The first file in this directory, or [`NO_ENTRY`] when there are none.
    pub file_offset: U32,
    /// The next entry in the same hash bucket, or [`NO_ENTRY`] at the end of the chain.
    pub hash_sibling_offset: U32,
    /// Length of the name following this entry, in bytes.
    pub name_len: U32,
    // The UTF-8 name follows inline, padded to a 4-byte boundary.
}

// Verify struct size - https://www.3dbrew.org/wiki/RomFS#Directory_Metadata_Structure
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
/// See <https://www.3dbrew.org/wiki/RomFS#File_Metadata_Structure>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct RomFsFileEntry {
    /// The directory this file sits in, as an offset into the directory metadata table.
    pub parent_offset: U32,
    /// The next file sharing this parent, or [`NO_ENTRY`] at the end of the chain.
    pub sibling_offset: U32,
    /// Offset of the file's first byte, from [`RomFsHeader::file_data_offset`].
    pub data_offset: U64,
    /// Length of the file in bytes.
    pub data_size: U64,
    /// The next entry in the same hash bucket, or [`NO_ENTRY`] at the end of the chain.
    pub hash_sibling_offset: U32,
    /// Length of the name following this entry, in bytes.
    pub name_len: U32,
    // The UTF-8 name follows inline, padded to a 4-byte boundary.
}

// Verify struct size - https://www.3dbrew.org/wiki/RomFS#File_Metadata_Structure
const_assert_eq!(size_of::<RomFsFileEntry>(), 0x20);
const_assert_eq!(align_of::<RomFsFileEntry>(), 0x1);

/// Hashes one path component to the bucket its entry is chained from.
///
/// An entry's bucket is this value modulo the number of buckets in the table, which the header
/// gives as a byte length rather than as a count. A reader hashes the name it is looking for and
/// walks that bucket's chain; a writer hashes each name it places and threads the chain the reader
/// will walk. The two have to agree exactly, which is why the arithmetic lives here rather than
/// once on each side.
///
/// `parent_offset` is the offset of the containing directory's entry, so the same name under two
/// parents lands in two different buckets. The name is taken as bytes because that is how it is
/// stored: the format calls it UTF-8, but hashing never decodes it, and a reader matching against
/// an image should not have to either.
///
/// This is not a general-purpose hash and has no property worth relying on beyond agreeing with
/// the format.
pub fn path_hash(parent_offset: u32, name: &[u8]) -> u32 {
    let mut hash = parent_offset ^ 123_456_789;
    for byte in name {
        hash = hash.rotate_right(5) ^ u32::from(*byte);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::path_hash;

    /// Offset of the root directory's entry, which is the first in its table.
    const ROOT: u32 = 0;

    #[test]
    fn path_hash_with_an_empty_name_returns_the_seeded_parent() {
        //* Given
        // A name with no bytes to fold in, so the seed is the whole result.
        let name = b"";

        //* When
        let hash = path_hash(ROOT, name);

        //* Then
        assert_eq!(
            hash, 123_456_789,
            "an empty name should hash to the parent offset seeded and otherwise untouched"
        );
    }

    #[test]
    fn path_hash_with_a_different_parent_returns_a_different_hash() {
        //* Given
        // One name, hashed under the root, against the directory whose entry
        // follows the root's one entry in.
        let name = b"data";
        let sibling_dir = 0x18;
        let under_root = path_hash(ROOT, name);

        //* When
        let under_sibling_dir = path_hash(sibling_dir, name);

        //* Then
        assert_ne!(
            under_sibling_dir, under_root,
            "the parent offset should reach the hash, so one name under two parents lands in two buckets"
        );
    }

    #[test]
    fn path_hash_with_a_multi_byte_name_hashes_every_byte() {
        //* Given
        // Four characters stored as twelve UTF-8 bytes.
        let name = "ファイル".as_bytes();

        //* When
        let hash = path_hash(ROOT, name);

        //* Then
        // Twelve rounds rather than four: hashing walks the stored bytes and never
        // decodes them into characters. The constant pins the rotation direction and
        // the order of the rotate and the XOR along with it.
        assert_eq!(
            hash, 0xF3B5_84EB,
            "a non-ASCII name should fold in one round per UTF-8 byte"
        );
    }
}
