//! The NCA header, its four section entries, and the per-section FS headers.
//!
//! An NCA opens with a fixed `0xC00`-byte header: two RSA-2048 signatures, the archive-wide fields,
//! a table of four section entries, and four [`NcaFsHeader`]s of `0x200` bytes each. Everything
//! after that is section data, and every offset that reaches it is a *media offset* — a count of
//! `0x200`-byte units from the start of the file, not a byte offset. [`MEDIA_UNIT_SIZE`] is the
//! conversion, and the header itself occupies the first six units.
//!
//! Each section entry says where a section lives; the [`NcaFsHeader`] at the same index says what is
//! in it. The two are matched by position and by nothing else, so a section written at index 1 whose
//! FS header is filled in at index 0 produces an archive the loader reads as empty.
//!
//! A section's FS header carries one of two superblocks in the same `0x138` bytes:
//! [`Pfs0Superblock`] for a flat archive, [`RomFsSuperblock`] for a hash-tree filesystem. Which one
//! is present is decided by `hash_type`, and nothing in the layout distinguishes them, which is why
//! this module models the choice as two separate structures rather than as a union.
//!
//! Nothing here is encrypted or decrypted. The structures describe the plaintext form; an NCA on
//! disk has its header wrapped in AES-XTS and, usually, its sections in AES-CTR. See the crate
//! documentation for why that transformation lives outside this crate.
//!
//! See <https://switchbrew.org/wiki/NCA>.

use static_assertions::const_assert_eq;
use zerocopy::little_endian::{U16, U32, U64};

/// Magic identifying an NCA3 header: `NCA3`, little-endian, at offset `0x200`.
pub const NCA3_MAGIC: u32 = 0x3341434E;

/// Magic identifying an IVFC header: `IVFC`, little-endian.
pub const IVFC_MAGIC: u32 = 0x43465649;

/// Bytes in the unit every offset in an NCA header is counted in.
///
/// Section bounds are stored as media offsets, so a section that starts `0x1000` bytes into the file
/// records a start of `8`. Sections are padded up to this size for the same reason: a bound that is
/// not a whole number of units cannot be expressed.
pub const MEDIA_UNIT_SIZE: u64 = 0x200;

/// Size of the NCA header, in bytes, and therefore the offset the first section may start at.
pub const NCA_HEADER_SIZE: u64 = 0xC00;

/// Number of sections an NCA header has room for.
pub const NCA_SECTION_COUNT: usize = 4;

/// Where one section begins and ends, in media units.
///
/// A section that is not present leaves its entry zeroed, which is why an all-zero entry means "no
/// section here" rather than "a section of length zero at the start of the file".
///
/// See <https://switchbrew.org/wiki/NCA#FsEntry>.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct NcaSectionEntry {
    /// First media unit of the section, counted from the start of the file.
    pub media_start_offset: U32,
    /// One past the last media unit of the section.
    pub media_end_offset: U32,
    /// Reserved by the format; the first byte is written as `1` for every populated entry.
    pub _reserved: [u8; 0x8],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#FsEntry
const_assert_eq!(size_of::<NcaSectionEntry>(), 0x10);
const_assert_eq!(align_of::<NcaSectionEntry>(), 0x1);

/// One level of the IVFC hash tree: where it sits and how large a block it hashes.
///
/// `logical_offset` is measured within the concatenated levels, not within the file, so level 0
/// starts at zero and each later level starts where the previous one ended.
///
/// See <https://switchbrew.org/wiki/NCA#HierarchicalIntegrityVerificationLevelInformation>.
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
pub struct IvfcLevelHeader {
    /// Offset of this level from the start of the first, in bytes.
    pub logical_offset: U64,
    /// Length of this level in bytes, including the padding it was rounded up with.
    pub hash_data_size: U64,
    /// Base-2 logarithm of the block size this level's hashes cover.
    pub block_size: U32,
    /// Unused by the format; zero in every image this crate writes.
    pub _reserved: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NCA#HierarchicalIntegrityVerificationLevelInformation
const_assert_eq!(size_of::<IvfcLevelHeader>(), 0x18);
const_assert_eq!(align_of::<IvfcLevelHeader>(), 0x1);

/// Number of level headers an IVFC header has room for.
pub const IVFC_MAX_LEVELS: usize = 6;

/// The root of a RomFS section's hash tree.
///
/// The tree is verified downwards: `master_hash` covers level 0 in full, level 0's blocks hash
/// level 1, and so on until the last level, which is the RomFS image itself. A level's own bytes are
/// therefore proven by the level above it, and only the master hash is proven by the header — which
/// is in turn covered by the FS header's hash in [`NcaHeader::section_hashes`].
///
/// `level_count` counts the levels *including* the master hash, so a six-level tree records seven.
///
/// See <https://switchbrew.org/wiki/NCA#IntegrityMetaInfo>.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct IvfcHeader {
    /// Always [`IVFC_MAGIC`].
    pub magic: U32,
    /// Format revision; `0x20000` in every image this crate writes.
    pub version: U32,
    /// Length of `master_hash` in bytes, which is `0x20` for the SHA-256 the format uses.
    pub master_hash_size: U32,
    /// Number of levels counting the master hash, so one more than the levels actually stored.
    ///
    /// The format calls this `MaxLayers`, and counts it as the first field of the `InfoLevelHash`
    /// block that also holds the level headers and the salt below.
    pub level_count: U32,
    /// The stored levels, low to high; entries past the ones in use are zero.
    pub level_headers: [IvfcLevelHeader; IVFC_MAX_LEVELS],
    /// Salt the format mixes into the tree's signature; zero in every image this crate writes,
    /// which is what an unsigned homebrew title carries.
    pub signature_salt: [u8; 0x20],
    /// SHA-256 of level 0 in full, padding included.
    pub master_hash: [u8; 0x20],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#IntegrityMetaInfo
const_assert_eq!(size_of::<IvfcHeader>(), 0xE0);
const_assert_eq!(align_of::<IvfcHeader>(), 0x1);

/// The FS-specific half of an [`NcaFsHeader`] for a RomFS section.
///
/// Padded out to the `0x138` bytes the FS header reserves, so that it and [`Pfs0Superblock`] occupy
/// the same span.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct RomFsSuperblock {
    /// The hash tree covering the section.
    pub ivfc_header: IvfcHeader,
    /// The rest of the span the FS header gives a superblock: the hash structure's own reserved
    /// tail, then the `PatchInfo` an update NCA would use. Zero for a base title.
    pub _reserved: [u8; 0x58],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#HashData
const_assert_eq!(size_of::<RomFsSuperblock>(), 0x138);
const_assert_eq!(align_of::<RomFsSuperblock>(), 0x1);

/// The FS-specific half of an [`NcaFsHeader`] for a PFS0 section.
///
/// A PFS0 section is a single-level hash table followed by the archive: the table holds one SHA-256
/// per `block_size` bytes of archive, and `master_hash` covers the table. Both regions are located
/// relative to the start of the section, so the offsets here are independent of where the section
/// landed in the file.
///
/// See <https://switchbrew.org/wiki/NCA#HierarchicalSha256Data>.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct Pfs0Superblock {
    /// SHA-256 of the hash table, over `hash_table_size` bytes and not its padding.
    pub master_hash: [u8; 0x20],
    /// Bytes of archive each entry in the hash table covers.
    pub block_size: U32,
    /// Number of layers the hash covers; always `2`, the table and the archive.
    pub layer_count: U32,
    /// Offset of the hash table from the start of the section; zero, as the table leads.
    ///
    /// This and the three fields below are the format's first two `LayerRegions`, each an offset
    /// and a size: one for the hash table, one for the archive.
    pub hash_table_offset: U64,
    /// Length of the hash table in bytes, excluding the padding that follows it.
    pub hash_table_size: U64,
    /// Offset of the archive from the start of the section, so the padded end of the table.
    pub pfs0_offset: U64,
    /// Length of the archive in bytes.
    pub pfs0_size: U64,
    /// The unused `LayerRegions` entries, the hash structure's reserved tail, and the `PatchInfo`
    /// an update NCA would use. Zero for a base title.
    pub _reserved: [u8; 0xF0],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#HashData
const_assert_eq!(size_of::<Pfs0Superblock>(), 0x138);
const_assert_eq!(align_of::<Pfs0Superblock>(), 0x1);

/// What kind of filesystem a section holds.
///
/// Stored in [`NcaFsHeader::fs_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NcaFsType {
    /// A RomFS image, verified by an IVFC hash tree.
    RomFs = 0,
    /// A flat PFS0 archive, verified by a single-level hash table.
    Pfs0 = 1,
}

/// Which verification structure the section's superblock describes.
///
/// Stored in [`NcaFsHeader::hash_type`]. It is this field, not `fs_type`, that says which of
/// [`Pfs0Superblock`] and [`RomFsSuperblock`] the superblock bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NcaHashType {
    /// A single-level hash table over a PFS0.
    Pfs0 = 2,
    /// An IVFC hash tree over a RomFS.
    RomFs = 3,
}

/// How a section's bytes are encrypted on disk.
///
/// Stored in [`NcaFsHeader::crypt_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NcaCryptType {
    /// Stored as written, with no encryption applied.
    None = 1,
    /// AES-XTS, which this crate's builders do not produce.
    Xts = 2,
    /// AES-CTR keyed from the key area, the usual choice for a section.
    Ctr = 3,
    /// AES-CTR with a relocation layer, used by update partitions only.
    Bktr = 4,
}

/// What an NCA holds, which is what the console decides how to mount it by.
///
/// Stored in [`NcaHeader::content_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NcaContentType {
    /// The executable and its assets.
    Program = 0,
    /// The content metadata describing every other NCA of the title.
    Meta = 1,
    /// The icon and the NACP.
    Control = 2,
    /// The manual, as HTML or as legal information.
    Manual = 3,
    /// A system data archive.
    Data = 4,
    /// A publicly readable system data archive.
    PublicData = 5,
}

/// The description of one section: what it holds, how it is verified, how it is encrypted.
///
/// Occupies `0x200` bytes at index `n` of [`NcaHeader::fs_headers`], and is covered by the SHA-256
/// at the same index of [`NcaHeader::section_hashes`] — so a field changed here without rehashing
/// invalidates the archive.
///
/// The superblock is `0x138` bytes whose meaning is fixed by `hash_type`; read it as
/// [`Pfs0Superblock`] or [`RomFsSuperblock`] accordingly.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct NcaFsHeader {
    /// Layout revision; always `2`.
    pub version: U16,
    /// One of [`NcaFsType`].
    pub fs_type: u8,
    /// One of [`NcaHashType`], and what decides how the superblock reads.
    pub hash_type: u8,
    /// One of [`NcaCryptType`].
    pub crypt_type: u8,
    /// Padding to the superblock.
    pub _reserved_0x5: [u8; 0x3],
    /// The verification structure, read according to `hash_type`, followed by the `PatchInfo` an
    /// update NCA would use. The format splits the span as `0xF8` of hash data and `0x40` of patch
    /// info; this crate writes the first and leaves the second zero.
    pub superblock: [u8; 0x138],
    /// High half of the AES-CTR counter, big-endian, for a section encrypted with CTR.
    ///
    /// The format names the two halves `Generation` and `SecureValue`; together they are what the
    /// counter for the section's first byte is built from.
    pub section_ctr: [u8; 0x8],
    /// Padding to `0x200`.
    pub _reserved_0x148: [u8; 0xB8],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#FsHeader
const_assert_eq!(size_of::<NcaFsHeader>(), 0x200);
const_assert_eq!(align_of::<NcaFsHeader>(), 0x1);

/// The whole `0xC00`-byte NCA header, in its plaintext form.
///
/// The two signatures lead: `fixed_key_sig` is checked against a modulus built into the console, and
/// `npdm_key_sig` against the public key in the program's own NPDM. Both cover the `0x200` bytes
/// from `magic` onwards — not the whole header — so the fields after `fs_headers` are outside what
/// either signature proves.
///
/// `encrypted_keys` is the key area: four AES-128 keys wrapped with the key area encryption key that
/// `crypto_type`, `crypto_type2` and `kaek_index` select between. This crate stores whatever bytes
/// it is handed there and never wraps them.
///
/// See <https://switchbrew.org/wiki/NCA#Header>.
#[derive(
    Debug,
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::KnownLayout,
    zerocopy::Immutable,
)]
#[repr(C)]
pub struct NcaHeader {
    /// RSA-2048-PSS over the `0x200` bytes at `magic`, under a fixed modulus.
    pub fixed_key_sig: [u8; 0x100],
    /// RSA-2048-PSS over the same `0x200` bytes, under the key in the program's NPDM.
    pub npdm_key_sig: [u8; 0x100],
    /// Always [`NCA3_MAGIC`].
    pub magic: U32,
    /// Where the archive is expected to be distributed from: `0` on-device, `1` gamecard.
    pub distribution: u8,
    /// One of [`NcaContentType`].
    pub content_type: u8,
    /// Key generation, low field; `2` or above once the key area needs a later keyset.
    pub crypto_type: u8,
    /// Which of the three key area encryption keys wrapped the key area.
    pub kaek_index: u8,
    /// Length of the whole archive in bytes, header included.
    pub nca_size: U64,
    /// Title this archive belongs to.
    pub title_id: U64,
    /// Unused by the format; zero in every image this crate writes.
    pub _reserved_0x218: [u8; 0x4],
    /// SDK the archive was built with, as `major.minor.micro.revision` packed into a word.
    pub sdk_version: U32,
    /// Key generation, high field; carries the generation once it exceeds what `crypto_type` holds.
    pub crypto_type2: u8,
    /// Unused by the format; zero in every image this crate writes.
    pub _reserved_0x221: [u8; 0xF],
    /// Rights ID for titlekey crypto; all zero when the key area is used instead.
    pub rights_id: [u8; 0x10],
    /// Where each section lives, in media units.
    pub section_entries: [NcaSectionEntry; NCA_SECTION_COUNT],
    /// SHA-256 of each [`NcaFsHeader`], in the same order.
    pub section_hashes: [[u8; 0x20]; NCA_SECTION_COUNT],
    /// The key area: four AES-128 keys, wrapped once the archive is finished.
    pub encrypted_keys: [[u8; 0x10]; NCA_SECTION_COUNT],
    /// Padding to the FS headers.
    pub _reserved_0x340: [u8; 0xC0],
    /// One description per section, matched to `section_entries` by index.
    pub fs_headers: [NcaFsHeader; NCA_SECTION_COUNT],
}

// Verify struct size - https://switchbrew.org/wiki/NCA#Header
const_assert_eq!(size_of::<NcaHeader>(), 0xC00);
const_assert_eq!(align_of::<NcaHeader>(), 0x1);
