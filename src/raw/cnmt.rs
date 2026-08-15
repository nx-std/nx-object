//! The content meta header, its application extended header, and the content records.
//!
//! A CNMT is what tells the console which NCAs make up a title. It is a header, an extended header
//! whose shape depends on the meta type, one record per content, and a `0x20`-byte digest closing
//! the file. Only the application meta type is described here, which is the one a homebrew title
//! ships as.
//!
//! The meta NCA that carries a CNMT does not list itself: the records name the program, the control,
//! and the manuals, and the meta NCA is found by the file that referenced the CNMT in the first
//! place.
//!
//! Sizes here are stored in six bytes rather than eight, so a content larger than `0xFFFFFFFFFFFF`
//! cannot be recorded. That is far beyond any title, but it is why [`CnmtContentRecord::size`] is a
//! byte array and not an integer.
//!
//! See <https://switchbrew.org/wiki/CNMT>.

use static_assertions::const_assert_eq;
use zerocopy::little_endian::{U16, U32, U64};

/// The fields every content meta file opens with, whatever its type.
///
/// `extended_header_size` is what locates the records: they begin that many bytes after this header,
/// so a size that does not match the extended header actually written shifts every record.
///
/// See <https://switchbrew.org/wiki/CNMT#PackagedContentMeta>.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct CnmtHeader {
    /// Title the described contents belong to.
    pub title_id: U64,
    /// Version of the title; zero for a first release.
    pub title_version: U32,
    /// One of [`CnmtContentMetaType`].
    pub meta_type: u8,
    /// Unused by the format; zero in every file this crate writes.
    pub _reserved_0xd: u8,
    /// Length of the extended header that follows, in bytes.
    pub extended_header_size: U16,
    /// Number of [`CnmtContentRecord`]s after the extended header.
    pub content_entry_count: U16,
    /// Number of meta records after the content records; zero for an application.
    pub meta_entry_count: U16,
    /// Unused by the format; zero in every file this crate writes.
    pub _reserved_0x14: [u8; 0xC],
}

// Verify struct size - https://switchbrew.org/wiki/CNMT#PackagedContentMeta
const_assert_eq!(size_of::<CnmtHeader>(), 0x20);
const_assert_eq!(align_of::<CnmtHeader>(), 0x1);

/// The extended header an application's content meta carries.
///
/// `patch_title_id` names where an update for this title would be published, which the console
/// derives rather than looks up: it is the title ID with `0x800` added.
///
/// See <https://switchbrew.org/wiki/CNMT#ApplicationMetaExtendedHeader>.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct CnmtApplicationExtendedHeader {
    /// Title ID updates for this title are published under.
    pub patch_title_id: U64,
    /// Lowest system version that may launch the title; zero imposes no floor.
    pub required_system_version: U32,
    /// Unused by the format; zero in every file this crate writes.
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/CNMT#ApplicationMetaExtendedHeader
const_assert_eq!(size_of::<CnmtApplicationExtendedHeader>(), 0x10);
const_assert_eq!(align_of::<CnmtApplicationExtendedHeader>(), 0x1);

/// One content of the title: which NCA it is, how large, and what it holds.
///
/// `nca_id` is not independent of `hash`: an NCA is named by the first sixteen bytes of its own
/// SHA-256, so the two fields agree by construction and a record whose ID is not the head of its
/// hash names a file that will not be found.
///
/// See <https://switchbrew.org/wiki/CNMT#PackagedContentInfo>.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct CnmtContentRecord {
    /// SHA-256 of the whole NCA file.
    pub hash: [u8; 0x20],
    /// The NCA's name on disk, which is the first sixteen bytes of `hash`.
    pub nca_id: [u8; 0x10],
    /// Length of the NCA in bytes, little-endian across six bytes.
    pub size: [u8; 0x6],
    /// One of [`CnmtContentType`].
    pub content_type: u8,
    /// Distinguishes several contents of one type; zero when there is only one.
    pub id_offset: u8,
}

// Verify struct size - https://switchbrew.org/wiki/CNMT#PackagedContentInfo
const_assert_eq!(size_of::<CnmtContentRecord>(), 0x38);
const_assert_eq!(align_of::<CnmtContentRecord>(), 0x1);

/// What kind of title the content meta describes.
///
/// Stored in [`CnmtHeader::meta_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CnmtContentMetaType {
    /// A base application, which is what a homebrew title ships as.
    Application = 0x80,
    /// An update to an application.
    Patch = 0x81,
    /// Downloadable content adding to an application.
    AddOnContent = 0x82,
    /// A delta between two versions of an application.
    Delta = 0x83,
}

/// What one content record points at.
///
/// Stored in [`CnmtContentRecord::content_type`]. The numbering is not the one
/// [`super::nca::NcaContentType`] uses, so the two must not be substituted for each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CnmtContentType {
    /// The content meta itself, which an application's records do not list.
    Meta = 0,
    /// The executable and its assets.
    Program = 1,
    /// A system data archive.
    Data = 2,
    /// The icon and the NACP.
    Control = 3,
    /// The manual, as HTML.
    HtmlDocument = 4,
    /// The legal information shown before launch.
    LegalInformation = 5,
    /// A delivery cache archive.
    DeltaFragment = 6,
}
