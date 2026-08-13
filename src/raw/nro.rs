//! The NRO start stub, header, and the optional asset section that follows it.
//!
//! An NRO begins with a branch stub rather than its magic: [`NroStart`] occupies
//! the first 0x10 bytes and [`NroHeader`]'s magic sits after it, so a reader that
//! checks offset zero for `NRO0` will not find it.
//!
//! The asset section is appended past the end the header reports and is optional:
//! its own `ASET` magic at that offset is what distinguishes an NRO carrying an
//! icon, NACP and RomFS from a bare one.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{U32, U64},
};

/// Magic identifying an NRO header: `NRO0`, little-endian.
///
/// It sits at offset `0x10`, not at the start of the file, because [`NroStart`] comes first.
pub const NRO_MAGIC: u32 = 0x304f524e;

/// Magic identifying the asset header appended past the end of an NRO: `ASET`, little-endian.
///
/// Its presence at [`NroHeader::size`] is what distinguishes an NRO carrying assets from a bare one.
pub const ASSET_MAGIC: u32 = 0x54455341;

/// Where one loadable segment sits in the file, and how far it extends.
///
/// The three segments of an NRO appear in [`NroHeader::segments`] in load order: `text`, `rodata`, `data`.
/// Each is mapped with its own permissions, so a segment's bounds decide which pages are executable.
///
/// See <https://switchbrew.org/wiki/NRO#Segments>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroSegment {
    /// Offset of the segment's first byte, from the start of the file.
    pub file_off: U32,
    /// Length of the segment in bytes.
    pub size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Segments
const_assert_eq!(size_of::<NroSegment>(), 0x8);
const_assert_eq!(align_of::<NroSegment>(), 0x1);

/// The first `0x10` bytes of an NRO, which the loader enters before any header is read.
///
/// The console jumps to offset zero, so the file opens with executable code rather than a magic.
/// A homebrew NRO puts its crt0 branch here; preserving those bytes is what keeps an NRO
/// launchable after a rewrite.
///
/// See <https://switchbrew.org/wiki/NRO#Start>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroStart {
    /// The instruction the loader enters at, in a homebrew NRO a branch past this header.
    ///
    /// Named for the role the format assigns it, which is none: the loader neither reads nor
    /// requires it, and only the entry stub gives it meaning.
    pub unused: U32,
    /// Offset of the MOD0 header, from the start of the file.
    pub mod_offset: U32,
    _padding: [u8; 8],
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Start
const_assert_eq!(size_of::<NroStart>(), 0x10);
const_assert_eq!(align_of::<NroStart>(), 0x1);

/// Everything the loader needs to map an NRO: its extent, its segments, and its identity.
///
/// Sits at offset `0x10`, immediately after [`NroStart`].
///
/// See <https://switchbrew.org/wiki/NRO#Header>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroHeader {
    /// Always [`NRO_MAGIC`]; anything else means this is not an NRO.
    pub magic: U32,
    /// Format revision. Every NRO in circulation carries `0`.
    pub version: U32,
    /// Length of the NRO proper, in bytes, which is also the offset any asset section starts at.
    ///
    /// Assets are appended past this point and are not counted here, so this is the file's length
    /// only for an NRO without them.
    pub size: U32,
    /// Loader flags. No bit is defined for the homebrew NROs this crate writes, so it is `0`.
    pub flags: U32,
    /// The `text`, `rodata`, and `data` segments, in that order and in load order.
    pub segments: [NroSegment; 3],
    /// Length of the zero-initialized region the loader appends after `data`, in bytes.
    ///
    /// The bytes are not stored in the file: this asks the loader to reserve and zero them.
    pub bss_size: U32,
    _reserved: U32,
    /// Identity of the linked binary, taken from the ELF build ID and zero-padded to `0x20` bytes.
    ///
    /// The console reports it on a crash, which is what makes it the link back to the build that
    /// produced the image.
    pub build_id: [u8; 0x20],
    _reserved2: [u8; 0x20],
}

// Verify struct size - https://switchbrew.org/wiki/NRO#Header
const_assert_eq!(size_of::<NroHeader>(), 0x70);
const_assert_eq!(align_of::<NroHeader>(), 0x1);

/// Where one asset sits within the asset section, and how far it extends.
///
/// A zero `size` means the asset is absent, which is how an NRO carries an icon but no RomFS.
///
/// See <https://switchbrew.org/wiki/NRO#AssetSection>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroAssetSection {
    /// Offset of the asset's first byte, from the start of [`NroAssetHeader`].
    ///
    /// Relative to the asset header, not to the file, so it survives the NRO growing in front of it.
    pub offset: U64,
    /// Length of the asset in bytes, or `0` when the asset is absent.
    pub size: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#AssetSection
const_assert_eq!(size_of::<NroAssetSection>(), 0x10);
const_assert_eq!(align_of::<NroAssetSection>(), 0x1);

/// The header of the asset section, carrying the homebrew menu's icon, metadata, and filesystem.
///
/// Begins at [`NroHeader::size`], past the end of the NRO the loader maps. The console ignores
/// everything here; it exists for the homebrew menu, which reads the icon and NACP to display an
/// entry and hands the RomFS to the running program.
///
/// See <https://switchbrew.org/wiki/NRO#AssetHeader>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NroAssetHeader {
    /// Always [`ASSET_MAGIC`]; anything else means no asset section is present.
    pub magic: U32,
    /// Asset format revision. Every asset section this crate reads or writes carries `0`.
    pub version: U32,
    /// The JPEG shown as the title's icon in the homebrew menu.
    pub icon: NroAssetSection,
    /// The NACP supplying the title, author, and version the menu displays.
    pub nacp: NroAssetSection,
    /// The RomFS image the running program mounts as its read-only filesystem.
    pub romfs: NroAssetSection,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#AssetHeader
const_assert_eq!(size_of::<NroAssetHeader>(), 0x38);
const_assert_eq!(align_of::<NroAssetHeader>(), 0x1);
