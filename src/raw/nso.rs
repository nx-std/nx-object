//! The NSO header, its segment descriptors, and the flags gating them.
//!
//! A segment's size is split across two places: [`NsoSegmentHeader::size`] is the
//! decompressed size, while the compressed length actually occupying the file is
//! held separately in the header's `*_file_size` fields. Which of the two applies
//! depends on the per-segment compression bit in `flags`, and the SHA-256 hashes
//! are taken over the decompressed bytes.
//!
//! The `.dynstr`, `.dynsym` and embedded-data offsets are relative to the start of
//! the rodata segment, not to the image.

use bitflags::bitflags;
use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::U32};

/// Magic identifying an NSO header: `NSO0`, little-endian, at offset zero.
pub const NSO_MAGIC: u32 = 0x304f534e;

bitflags! {
    /// Which segments are LZ4-compressed in the file, and which the loader hash-checks on load.
    ///
    /// The two halves are independent: a segment can be stored compressed without being verified,
    /// and verified without being compressed. A compression bit is what decides whether a segment's
    /// bytes in the file are its contents or its compressed form, so clearing one without
    /// rewriting the segment leaves an image the loader reads as garbage.
    #[derive(Debug, Clone, Copy)]
    pub struct NsoFlags: u32 {
        /// The `text` segment is stored LZ4-compressed.
        const TEXT_COMPRESS = 1 << 0;
        /// The `rodata` segment is stored LZ4-compressed.
        const RODATA_COMPRESS = 1 << 1;
        /// The `data` segment is stored LZ4-compressed.
        const DATA_COMPRESS = 1 << 2;
        /// The loader checks `text` against [`NsoHeader::text_hash`] before mapping it.
        const TEXT_HASH = 1 << 3;
        /// The loader checks `rodata` against [`NsoHeader::rodata_hash`] before mapping it.
        const RODATA_HASH = 1 << 4;
        /// The loader checks `data` against [`NsoHeader::data_hash`] before mapping it.
        const DATA_HASH = 1 << 5;
    }
}

/// Where one segment sits in the file, where it lands in memory, and how large it is once expanded.
///
/// See <https://switchbrew.org/wiki/NSO#NsoHeader>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NsoSegmentHeader {
    /// Offset of the segment's first stored byte, from the start of the file.
    pub file_offset: U32,
    /// Offset the segment is mapped at, from the start of the image.
    pub memory_offset: U32,
    /// Length of the segment after decompression, in bytes.
    ///
    /// This is the mapped length, not the stored one. What the segment occupies in the file is
    /// the matching `*_file_size` field of [`NsoHeader`], and the two differ whenever the
    /// segment's compression bit in [`NsoFlags`] is set.
    pub size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NSO#NsoHeader
const_assert_eq!(size_of::<NsoSegmentHeader>(), 0xC);
const_assert_eq!(align_of::<NsoSegmentHeader>(), 0x1);

/// Everything the loader needs to map an NSO: its segments, their stored lengths, and their hashes.
///
/// Occupies the first `0x100` bytes of the file. Unlike an NRO, an NSO opens with its magic rather
/// than with code, and its segments may be compressed and verified individually.
///
/// See <https://switchbrew.org/wiki/NSO#NsoHeader>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NsoHeader {
    /// Always [`NSO_MAGIC`]; anything else means this is not an NSO.
    pub magic: U32,
    /// Format revision. Every NSO this crate reads or writes carries `0`.
    pub version: U32,
    _reserved: U32,
    /// The [`NsoFlags`] bits deciding which segments are compressed and which are hash-checked.
    pub flags: U32,
    /// Bounds of the `text` segment, holding executable code.
    pub text: NsoSegmentHeader,
    /// Offset of the module name string, from the start of the file.
    pub module_name_offset: U32,
    /// Bounds of the `rodata` segment, holding constants and the dynamic linking tables.
    pub rodata: NsoSegmentHeader,
    /// Length of the module name string in bytes.
    pub module_name_size: U32,
    /// Bounds of the `data` segment, holding writable initialized data.
    pub data: NsoSegmentHeader,
    /// Length of the zero-initialized region the loader appends after `data`, in bytes.
    pub bss_size: U32,
    /// Identity of the linked binary, taken from the ELF build ID and zero-padded to `0x20` bytes.
    pub module_id: [u8; 0x20],
    /// Bytes the `text` segment occupies in the file, compressed if `TEXT_COMPRESS` is set.
    pub text_file_size: U32,
    /// Bytes the `rodata` segment occupies in the file, compressed if `RODATA_COMPRESS` is set.
    pub rodata_file_size: U32,
    /// Bytes the `data` segment occupies in the file, compressed if `DATA_COMPRESS` is set.
    pub data_file_size: U32,
    _reserved2: [u8; 0x1C],
    /// Offset of the embedded data blob, from the start of the `rodata` segment.
    pub embedded_offset: U32,
    /// Length of the embedded data blob in bytes.
    pub embedded_size: U32,
    /// Offset of the `.dynstr` table, from the start of the `rodata` segment.
    pub dynstr_offset: U32,
    /// Length of the `.dynstr` table in bytes.
    pub dynstr_size: U32,
    /// Offset of the `.dynsym` table, from the start of the `rodata` segment.
    pub dynsym_offset: U32,
    /// Length of the `.dynsym` table in bytes.
    pub dynsym_size: U32,
    /// SHA-256 of the `text` segment, taken over its decompressed bytes.
    pub text_hash: [u8; 0x20],
    /// SHA-256 of the `rodata` segment, taken over its decompressed bytes.
    pub rodata_hash: [u8; 0x20],
    /// SHA-256 of the `data` segment, taken over its decompressed bytes.
    pub data_hash: [u8; 0x20],
}

// Verify struct size - https://switchbrew.org/wiki/NSO#NsoHeader
const_assert_eq!(size_of::<NsoHeader>(), 0x100);
const_assert_eq!(align_of::<NsoHeader>(), 0x1);
