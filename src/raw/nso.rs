use bitflags::bitflags;
use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// NSO magic number: "NSO0" in ASCII (0x304f534e).
pub const NSO_MAGIC: u32 = 0x304f534e;

bitflags! {
    /// NSO header flags indicating compression and hash validation.
    #[derive(Debug, Clone, Copy)]
    pub struct NsoFlags: u32 {
        /// Text segment is compressed
        const TEXT_COMPRESS = 1 << 0;
        /// Rodata segment is compressed
        const RODATA_COMPRESS = 1 << 1;
        /// Data segment is compressed
        const DATA_COMPRESS = 1 << 2;
        /// Text segment hash should be checked
        const TEXT_HASH = 1 << 3;
        /// Rodata segment hash should be checked
        const RODATA_HASH = 1 << 4;
        /// Data segment hash should be checked
        const DATA_HASH = 1 << 5;
    }
}

/// NSO segment header for text, rodata, or data segments.
///
/// Contains file offset, memory offset, and decompressed size.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NsoSegmentHeader {
    /// Offset to segment data within NSO file
    pub file_offset: U32,
    /// Offset where segment should be loaded in memory
    pub memory_offset: U32,
    /// Size of segment after decompression
    pub size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NSO#Segment_Header
const_assert_eq!(size_of::<NsoSegmentHeader>(), 0xC);
const_assert_eq!(align_of::<NsoSegmentHeader>(), 0x1);

/// NSO header (0x100 bytes) - official software module format.
///
/// NSO files contain compressed and hashed segments of executable code.
/// Used for official system modules and game code.
///
/// See: <https://switchbrew.org/wiki/NSO>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NsoHeader {
    /// Magic number (must be `NSO_MAGIC`)
    pub magic: U32,
    /// Format version
    pub version: U32,
    /// Reserved
    _reserved: U32,
    /// Flags (compression and hash validation)
    pub flags: U32,
    /// Text segment header
    pub text: NsoSegmentHeader,
    /// Offset to module name string
    pub module_name_offset: U32,
    /// Rodata segment header
    pub rodata: NsoSegmentHeader,
    /// Length of module name string
    pub module_name_size: U32,
    /// Data segment header
    pub data: NsoSegmentHeader,
    /// BSS section size in bytes
    pub bss_size: U32,
    /// 32-byte module ID (build ID)
    pub module_id: [u8; 0x20],
    /// Compressed text segment size in file
    pub text_file_size: U32,
    /// Compressed rodata segment size in file
    pub rodata_file_size: U32,
    /// Compressed data segment size in file
    pub data_file_size: U32,
    /// Reserved
    _reserved2: [u8; 0x1C],
    /// Offset to embedded data (relative to rodata)
    pub embedded_offset: U32,
    /// Size of embedded data
    pub embedded_size: U32,
    /// Offset to .dynstr section (relative to rodata)
    pub dynstr_offset: U32,
    /// Size of .dynstr section
    pub dynstr_size: U32,
    /// Offset to .dynsym section (relative to rodata)
    pub dynsym_offset: U32,
    /// Size of .dynsym section
    pub dynsym_size: U32,
    /// SHA256 hash of decompressed text segment
    pub text_hash: [u8; 0x20],
    /// SHA256 hash of decompressed rodata segment
    pub rodata_hash: [u8; 0x20],
    /// SHA256 hash of decompressed data segment
    pub data_hash: [u8; 0x20],
}

// Verify struct size - https://switchbrew.org/wiki/NSO#Header
const_assert_eq!(size_of::<NsoHeader>(), 0x100);
const_assert_eq!(align_of::<NsoHeader>(), 0x1);
