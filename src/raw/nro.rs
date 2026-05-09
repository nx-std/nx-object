use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// NRO magic number: "NRO0" in ASCII (0x304f524e).
pub const NRO_MAGIC: u32 = 0x304f524e;

/// ASET magic number: "ASET" in ASCII (0x54455341).
///
/// Used in the asset header appended to NRO files.
pub const ASSET_MAGIC: u32 = 0x54455341;

/// NRO segment descriptor (text, rodata, or data).
///
/// Describes location and size of a loaded segment within the NRO file.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroSegment {
    /// File offset to segment data
    pub file_off: U32,
    /// Size of segment in bytes
    pub size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Segments
const_assert_eq!(size_of::<NroSegment>(), 0x8);
const_assert_eq!(align_of::<NroSegment>(), 0x1);

/// NRO start header (first 0x10 bytes of NRO file).
///
/// Contains branch instruction and offset to MOD0 header.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroStart {
    /// Unused (typically branch instruction in actual NRO)
    pub unused: U32,
    /// Offset to MOD0 header (relative to NRO start)
    pub mod_offset: U32,
    /// Padding
    _padding: [u8; 8],
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Start
const_assert_eq!(size_of::<NroStart>(), 0x10);
const_assert_eq!(align_of::<NroStart>(), 0x1);

/// NRO header (0x70 bytes, follows NroStart at offset 0x10).
///
/// Contains segment descriptors, build ID, and metadata about the NRO file.
///
/// See: <https://switchbrew.org/wiki/NRO>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroHeader {
    /// Magic number (must be [`NRO_MAGIC`])
    pub magic: U32,
    /// Format version (usually 0)
    pub version: U32,
    /// Total size of NRO file (excluding assets)
    pub size: U32,
    /// Flags
    pub flags: U32,
    /// Array of 3 segment descriptors: [text, rodata, data]
    pub segments: [NroSegment; 3],
    /// BSS section size in bytes
    pub bss_size: U32,
    /// Reserved
    _reserved: U32,
    /// 32-byte build ID
    pub build_id: [u8; 0x20],
    /// Reserved
    _reserved2: [u8; 0x20],
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Header
const_assert_eq!(size_of::<NroHeader>(), 0x70);
const_assert_eq!(align_of::<NroHeader>(), 0x1);

/// NRO asset section descriptor.
///
/// Describes location and size of an asset (icon, NACP, or RomFS) appended to the NRO.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroAssetSection {
    /// Offset to asset data (relative to asset header)
    pub offset: U64,
    /// Size of asset data in bytes
    pub size: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#AssetSection
const_assert_eq!(size_of::<NroAssetSection>(), 0x10);
const_assert_eq!(align_of::<NroAssetSection>(), 0x1);

/// NRO asset header (appended after NRO file data).
///
/// Contains descriptors for optional assets: icon, NACP, and RomFS.
/// This header is located immediately after the main NRO data.
///
/// See: <https://switchbrew.org/wiki/NRO#AssetHeader>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroAssetHeader {
    /// Magic number (must be `ASSET_MAGIC`)
    pub magic: U32,
    /// Asset format version
    pub version: U32,
    /// Icon asset descriptor (JPEG image)
    pub icon: NroAssetSection,
    /// NACP asset descriptor
    pub nacp: NroAssetSection,
    /// RomFS asset descriptor
    pub romfs: NroAssetSection,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#AssetHeader
const_assert_eq!(size_of::<NroAssetHeader>(), 0x38);
const_assert_eq!(align_of::<NroAssetHeader>(), 0x1);
