//! Builder for the NCA container, in its plaintext form.
//!
//! An NCA is a `0xC00`-byte header followed by up to four sections. This builder lays the sections
//! out, derives the hash structures that verify them, and fills in the header that describes them —
//! and stops there. What it returns is a [`PlainNca`]: correct in every field, and not yet an
//! archive the console will load.
//!
//! Three transformations are still owed, and all three need key material this crate deliberately
//! does not handle:
//!
//! 1. Wrap the key area in [`crate::raw::nca::NcaHeader::encrypted_keys`] with the key area
//!    encryption key the header's generation selects.
//! 2. Encrypt each section [`PlainNca::ctr_sections`] names, using the key area's third key and the
//!    counter given for it.
//! 3. Encrypt the header itself with AES-XTS under the header key, then write it ahead of the body.
//!
//! They must happen in that order: the section counters are derived from the plaintext header, and
//! the header cannot be encrypted until the signatures over it are in place. See the crate
//! documentation for why the crypto lives outside this crate rather than behind a feature of it.

use alloc::{vec, vec::Vec};
use core::ops::Range;

use sha2::{Digest as _, Sha256};
use zerocopy::IntoBytes as _;

pub mod ivfc;
pub mod partition;

use crate::raw::nca::{
    IVFC_MAGIC, MEDIA_UNIT_SIZE, NCA_HEADER_SIZE, NCA_SECTION_COUNT, NCA3_MAGIC, NcaContentType,
    NcaCryptType, NcaFsHeader, NcaFsType, NcaHashType, NcaHeader, NcaSectionEntry,
};

/// Highest key generation the header's two generation fields can express.
const MAX_KEY_GENERATION: u8 = 32;

/// Index of the key in the key area that a CTR-encrypted section is encrypted with.
///
/// The other three slots are unused by a homebrew title, so this is the only one a caller fills in
/// and the only one whose value reaches a cipher.
pub const SECTION_KEY_INDEX: usize = 2;

/// Which keyset generation the key area is wrapped with.
///
/// The header splits the generation across two fields with a gap in the numbering, so it is stored
/// here as the single number a caller thinks in and split only when the header is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyGeneration(u8);

impl KeyGeneration {
    /// The generation every title used before the keyset was first rotated.
    pub const FIRST: Self = Self(1);

    /// The generation as the number a keyset file names it by.
    pub fn to_u8(self) -> u8 {
        self.0
    }
}

impl core::fmt::Display for KeyGeneration {
    /// Renders as the plain decimal number a keyset file names the generation by, so the first
    /// generation prints as `1`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u8> for KeyGeneration {
    type Error = ParseKeyGenerationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 0 || value > MAX_KEY_GENERATION {
            return Err(ParseKeyGenerationError::OutOfRange {
                value: value.into(),
            });
        }
        Ok(Self(value))
    }
}

impl core::str::FromStr for KeyGeneration {
    type Err = ParseKeyGenerationError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let value = input
            .parse::<u32>()
            .map_err(|_| ParseKeyGenerationError::NotANumber)?;
        let value =
            u8::try_from(value).map_err(|_| ParseKeyGenerationError::OutOfRange { value })?;
        Self::try_from(value)
    }
}

/// Error returned when a key generation cannot be built.
#[derive(Debug, thiserror::Error)]
pub enum ParseKeyGenerationError {
    /// The input is not a decimal number.
    #[error("key generation must be a decimal number")]
    NotANumber,
    /// The generation is zero or above what the header can express.
    ///
    /// Holds the rejected value.
    #[error("key generation {value} is outside the range 1-{MAX_KEY_GENERATION}")]
    OutOfRange {
        /// The rejected value.
        value: u32,
    },
}

/// Whether a section's bytes are encrypted once the archive is finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionEncryption {
    /// Written as-is. The section's FS header records plaintext, so nothing decrypts it on load.
    None,
    /// Encrypted with AES-CTR by the caller, keyed from the key area.
    Ctr,
}

/// What one section holds, and therefore how it is verified.
pub enum SectionData {
    /// A RomFS image, which gains an IVFC hash tree ahead of it.
    RomFs(Vec<u8>),
    /// A PFS0 archive, which gains a hash table ahead of it.
    Partition {
        /// The archive as [`super::Pfs0Builder`] produced it.
        archive: Vec<u8>,
        /// Bytes of archive each entry in the hash table covers.
        hash_block_size: u32,
    },
}

/// One section of the archive: its contents and how it is encrypted.
pub struct Section {
    /// What the section holds.
    pub data: SectionData,
    /// How the section's bytes are protected.
    pub encryption: SectionEncryption,
}

/// Assembles sections and the header describing them into a plaintext NCA.
pub struct NcaBuilder {
    content_type: NcaContentType,
    title_id: u64,
    sdk_version: u32,
    key_generation: KeyGeneration,
    key_area: [[u8; 0x10]; NCA_SECTION_COUNT],
    sections: [Option<Section>; NCA_SECTION_COUNT],
}

impl NcaBuilder {
    /// Start an archive of `content_type` for `title_id`, holding no sections.
    ///
    /// The SDK version defaults to none and the key generation to [`KeyGeneration::FIRST`]; the key
    /// area is all zeroes until [`NcaBuilder::key_area_key`] fills a slot in.
    pub fn new(content_type: NcaContentType, title_id: u64) -> Self {
        Self {
            content_type,
            title_id,
            sdk_version: 0,
            key_generation: KeyGeneration::FIRST,
            key_area: [[0; 0x10]; NCA_SECTION_COUNT],
            sections: [const { None }; NCA_SECTION_COUNT],
        }
    }

    /// Record the SDK the archive claims to have been built with.
    pub fn sdk_version(mut self, sdk_version: u32) -> Self {
        self.sdk_version = sdk_version;
        self
    }

    /// Set the keyset generation the key area is wrapped with.
    pub fn key_generation(mut self, key_generation: KeyGeneration) -> Self {
        self.key_generation = key_generation;
        self
    }

    /// Place `key` at `index` of the key area, unwrapped.
    ///
    /// Index [`SECTION_KEY_INDEX`] is the one a CTR-encrypted section is encrypted with; the others
    /// are unused by a homebrew title and stay zero.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is not one of the four slots the key area holds.
    pub fn key_area_key(mut self, index: usize, key: [u8; 0x10]) -> Result<Self, AddSectionError> {
        let slot = self
            .key_area
            .get_mut(index)
            .ok_or(AddSectionError::IndexOutOfRange { index })?;
        *slot = key;
        Ok(self)
    }

    /// Place `section` at `index`.
    ///
    /// Sections are addressed by position rather than appended, because a section's entry and its FS
    /// header are matched by index and a title may leave a slot empty — a program with no RomFS
    /// still carries its logo at index 2.
    ///
    /// # Errors
    ///
    /// Returns an error if `index` is not one of the four slots an NCA holds, or if a section was
    /// already placed there.
    pub fn section(mut self, index: usize, section: Section) -> Result<Self, AddSectionError> {
        let slot = self
            .sections
            .get_mut(index)
            .ok_or(AddSectionError::IndexOutOfRange { index })?;

        if slot.is_some() {
            return Err(AddSectionError::IndexOccupied { index });
        }

        *slot = Some(section);
        Ok(self)
    }

    /// Lay the sections out, derive their hashes, and return the archive with its header filled in.
    ///
    /// # Errors
    ///
    /// Returns an error if the assembled archive is larger than the header's media offsets can
    /// address, which is a little over 2 TiB.
    pub fn build(self) -> Result<PlainNca, BuildError> {
        let mut header = NcaHeader {
            fixed_key_sig: [0; 0x100],
            npdm_key_sig: [0; 0x100],
            magic: NCA3_MAGIC.into(),
            distribution: 0,
            content_type: self.content_type as u8,
            crypto_type: 0,
            kaek_index: 0,
            nca_size: 0.into(),
            title_id: self.title_id.into(),
            _reserved_0x218: [0; 0x4],
            sdk_version: self.sdk_version.into(),
            crypto_type2: 0,
            _reserved_0x221: [0; 0xF],
            rights_id: [0; 0x10],
            section_entries: [EMPTY_SECTION_ENTRY; NCA_SECTION_COUNT],
            section_hashes: [[0; 0x20]; NCA_SECTION_COUNT],
            encrypted_keys: self.key_area,
            _reserved_0x340: [0; 0xC0],
            fs_headers: [EMPTY_FS_HEADER; NCA_SECTION_COUNT],
        };

        let (low, high) = split_key_generation(self.key_generation);
        header.crypto_type = low;
        header.crypto_type2 = high;

        let mut body = Vec::new();
        for (index, section) in self.sections.into_iter().enumerate() {
            let Some(section) = section else {
                continue;
            };

            let media_start = media_offset(NCA_HEADER_SIZE + body.len() as u64)?;
            let (data, fs_header) = lay_out_section(section);
            body.extend_from_slice(&data);
            pad_to_media_unit(&mut body);
            let media_end = media_offset(NCA_HEADER_SIZE + body.len() as u64)?;

            // `index` comes from a four-element array, so it is in range for every table here.
            header.section_entries[index] = NcaSectionEntry {
                media_start_offset: media_start.into(),
                media_end_offset: media_end.into(),
                _reserved: [1, 0, 0, 0, 0, 0, 0, 0],
            };
            header.section_hashes[index] = Sha256::digest(fs_header.as_bytes()).into();
            header.fs_headers[index] = fs_header;
        }

        header.nca_size = (NCA_HEADER_SIZE + body.len() as u64).into();

        Ok(PlainNca { header, body })
    }
}

/// A finished archive that still needs its key area wrapped, its sections encrypted, and its header
/// encrypted and written ahead of the body.
///
/// See the module documentation for the order those three steps must happen in.
pub struct PlainNca {
    /// The header, complete but for the signatures, and not yet encrypted.
    ///
    /// [`crate::raw::nca::NcaHeader::encrypted_keys`] holds the key area as it was handed to the
    /// builder — unwrapped, despite the field's name, which is the format's.
    pub header: NcaHeader,
    /// Everything after the header, as the sections were laid out.
    pub body: Vec<u8>,
}

impl PlainNca {
    /// The sections that must be encrypted before the archive is valid.
    ///
    /// Each range indexes [`PlainNca::body`], and each counter is the AES-CTR counter for the first
    /// byte of its range. The key is the one at index 2 of the key area, taken before it is wrapped.
    pub fn ctr_sections(&self) -> Vec<CtrSection> {
        let mut sections = Vec::new();

        for (entry, fs_header) in self
            .header
            .section_entries
            .iter()
            .zip(self.header.fs_headers.iter())
        {
            if fs_header.crypt_type != NcaCryptType::Ctr as u8 {
                continue;
            }

            let start = u64::from(entry.media_start_offset.get()) * MEDIA_UNIT_SIZE;
            let end = u64::from(entry.media_end_offset.get()) * MEDIA_UNIT_SIZE;

            // Both bounds index a body already held in memory, so they fit `usize` by construction.
            sections.push(CtrSection {
                range: (start - NCA_HEADER_SIZE) as usize..(end - NCA_HEADER_SIZE) as usize,
                counter: section_counter(&fs_header.section_ctr, start),
            });
        }

        sections
    }

    /// The whole archive, header first, as it stands.
    ///
    /// Call this once the header has been signed and encrypted and the sections encrypted in place;
    /// called before that, it returns a plaintext archive the console will refuse.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut image = Vec::with_capacity(NCA_HEADER_SIZE as usize + self.body.len());
        image.extend_from_slice(self.header.as_bytes());
        image.extend_from_slice(&self.body);
        image
    }
}

/// One section awaiting AES-CTR encryption.
pub struct CtrSection {
    /// Where the section sits within [`PlainNca::body`].
    pub range: Range<usize>,
    /// The counter for the first byte of `range`.
    pub counter: [u8; 0x10],
}

/// An empty section entry, which is what an unused slot holds.
const EMPTY_SECTION_ENTRY: NcaSectionEntry = NcaSectionEntry {
    media_start_offset: zerocopy::little_endian::U32::ZERO,
    media_end_offset: zerocopy::little_endian::U32::ZERO,
    _reserved: [0; 0x8],
};

/// An empty FS header, which is what an unused slot holds.
const EMPTY_FS_HEADER: NcaFsHeader = NcaFsHeader {
    version: zerocopy::little_endian::U16::ZERO,
    fs_type: 0,
    hash_type: 0,
    crypt_type: 0,
    _reserved_0x5: [0; 0x3],
    superblock: [0; 0x138],
    section_ctr: [0; 0x8],
    _reserved_0x148: [0; 0xB8],
};

/// Turn a section into the bytes it occupies and the FS header describing them.
fn lay_out_section(section: Section) -> (Vec<u8>, NcaFsHeader) {
    let mut fs_header = EMPTY_FS_HEADER;
    fs_header.version = 2.into();
    fs_header.crypt_type = match section.encryption {
        SectionEncryption::None => NcaCryptType::None as u8,
        SectionEncryption::Ctr => NcaCryptType::Ctr as u8,
    };

    let data = match section.data {
        SectionData::RomFs(image) => {
            let tree = ivfc::build(image);

            let mut superblock = crate::raw::nca::RomFsSuperblock {
                ivfc_header: crate::raw::nca::IvfcHeader {
                    magic: IVFC_MAGIC.into(),
                    version: 0x20000.into(),
                    master_hash_size: 0x20.into(),
                    // Counts the master hash as well as the stored levels.
                    level_count: (tree.level_headers.len() as u32 + 1).into(),
                    level_headers: tree.level_headers,
                    signature_salt: [0; 0x20],
                    master_hash: tree.master_hash,
                },
                _reserved: [0; 0x58],
            };
            // `fs_type` stays RomFs, which is zero, so only the hash type is set here.
            fs_header.fs_type = NcaFsType::RomFs as u8;
            fs_header.hash_type = NcaHashType::RomFs as u8;
            fs_header
                .superblock
                .copy_from_slice(superblock.as_mut_bytes());

            tree.data
        }
        SectionData::Partition {
            archive,
            hash_block_size,
        } => {
            let mut built = partition::build(archive, hash_block_size);

            fs_header.fs_type = NcaFsType::Pfs0 as u8;
            fs_header.hash_type = NcaHashType::Pfs0 as u8;
            fs_header
                .superblock
                .copy_from_slice(built.superblock.as_mut_bytes());

            built.data
        }
    };

    (data, fs_header)
}

/// Split a key generation across the header's two generation fields.
///
/// The low field saturates at 2 and the high field carries anything beyond it, which is the shape
/// the format ended up with after the first rotation.
fn split_key_generation(key_generation: KeyGeneration) -> (u8, u8) {
    match key_generation.to_u8() {
        1 => (0, 0),
        2 => (2, 0),
        generation => (2, generation),
    }
}

/// Convert a byte offset into the media units a section entry records.
fn media_offset(offset: u64) -> Result<u32, BuildError> {
    let units = offset / MEDIA_UNIT_SIZE;
    u32::try_from(units).map_err(|_| BuildError { size: offset })
}

/// Pad `body` up to the next media unit, leaving it untouched if it is already on one.
fn pad_to_media_unit(body: &mut Vec<u8>) {
    let remainder = body.len() as u64 % MEDIA_UNIT_SIZE;
    if remainder != 0 {
        body.extend_from_slice(&vec![0u8; (MEDIA_UNIT_SIZE - remainder) as usize]);
    }
}

/// Build the AES-CTR counter for the byte at `offset` of a section.
///
/// The high half is the FS header's counter reversed, and the low half is the offset in AES blocks,
/// big-endian — so a section decrypts the same whichever byte of it is read first.
fn section_counter(section_ctr: &[u8; 0x8], offset: u64) -> [u8; 0x10] {
    let mut counter = [0u8; 0x10];
    for (index, byte) in section_ctr.iter().rev().enumerate() {
        counter[index] = *byte;
    }
    counter[0x8..].copy_from_slice(&(offset / 0x10).to_be_bytes());
    counter
}

/// Error returned by [`NcaBuilder::section`] and [`NcaBuilder::key_area_key`].
#[derive(Debug, thiserror::Error)]
pub enum AddSectionError {
    /// The index is not one of the four slots an NCA header holds.
    ///
    /// Holds the rejected index.
    #[error("index {index} is outside the {NCA_SECTION_COUNT} an NCA holds")]
    IndexOutOfRange {
        /// The rejected index.
        index: usize,
    },
    /// A section was already placed at this index.
    ///
    /// Holds the occupied index.
    #[error("a section was already placed at index {index}")]
    IndexOccupied {
        /// The occupied index.
        index: usize,
    },
}

/// The archive is larger than the header's media offsets can address.
///
/// Returned by [`NcaBuilder::build`], which fails in no other way: the sections were already checked
/// as they were placed, and laying them out cannot go wrong once they fit.
///
#[derive(Debug, thiserror::Error)]
#[error("an archive of {size} bytes exceeds what a media offset can address")]
pub struct BuildError {
    /// The size that could not be recorded.
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::{
        KeyGeneration, NcaBuilder, Section, SectionData, SectionEncryption, section_counter,
    };
    use crate::raw::nca::{MEDIA_UNIT_SIZE, NCA_HEADER_SIZE, NcaContentType, NcaCryptType};

    /// A PFS0-shaped section holding `len` bytes of filler.
    fn partition_section(len: usize, encryption: SectionEncryption) -> Section {
        Section {
            data: SectionData::Partition {
                archive: vec![0x33u8; len],
                hash_block_size: 0x10000,
            },
            encryption,
        }
    }

    #[test]
    fn build_starts_the_first_section_right_after_the_header() {
        //* Given
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .section(0, partition_section(0x400, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        assert_eq!(
            u64::from(nca.header.section_entries[0].media_start_offset.get()) * MEDIA_UNIT_SIZE,
            NCA_HEADER_SIZE,
            "the first section begins where the header ends"
        );
    }

    #[test]
    fn build_leaves_a_skipped_index_zeroed() {
        //* Given
        // A program with no RomFS still carries its logo at index 2.
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .section(0, partition_section(0x400, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed")
            .section(2, partition_section(0x200, SectionEncryption::None))
            .expect("placing a section at index 2 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        assert_eq!(
            nca.header.section_entries[1].media_end_offset.get(),
            0,
            "the skipped slot should stay empty"
        );
        assert_ne!(
            nca.header.section_entries[2].media_end_offset.get(),
            0,
            "the logo should still land at index 2"
        );
    }

    #[test]
    fn build_names_only_the_ctr_sections_as_needing_encryption() {
        //* Given
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .section(0, partition_section(0x400, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed")
            .section(2, partition_section(0x200, SectionEncryption::None))
            .expect("placing a section at index 2 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        let pending = nca.ctr_sections();
        assert_eq!(pending.len(), 1, "only the CTR section needs encrypting");
        assert_eq!(
            nca.header.fs_headers[2].crypt_type,
            NcaCryptType::None as u8,
            "the plaintext section should record plaintext"
        );
    }

    #[test]
    fn build_records_a_size_that_covers_the_header_and_the_body() {
        //* Given
        let builder = NcaBuilder::new(NcaContentType::Control, 0x0100000000001000)
            .section(0, partition_section(0x400, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        assert_eq!(
            nca.header.nca_size.get(),
            NCA_HEADER_SIZE + nca.body.len() as u64,
            "the recorded size is the whole file"
        );
        assert_eq!(nca.to_bytes().len() as u64, nca.header.nca_size.get());
    }

    #[test]
    fn section_at_an_occupied_index_fails() {
        //* Given
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .section(0, partition_section(0x200, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed");

        //* When
        let result = builder.section(0, partition_section(0x200, SectionEncryption::Ctr));

        //* Then
        assert!(
            result.is_err(),
            "one index describes one section, so the second must be refused"
        );
    }

    #[test]
    fn build_with_key_generation_one_leaves_both_header_fields_clear() {
        //* Given
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .key_generation(KeyGeneration::FIRST)
            .section(0, partition_section(0x200, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        assert_eq!(nca.header.crypto_type, 0);
        assert_eq!(nca.header.crypto_type2, 0);
    }

    #[test]
    fn build_with_a_key_generation_above_two_carries_it_in_the_high_field() {
        //* Given
        let generation = KeyGeneration::try_from(11).expect("11 is a valid generation");
        let builder = NcaBuilder::new(NcaContentType::Program, 0x0100000000001000)
            .key_generation(generation)
            .section(0, partition_section(0x200, SectionEncryption::Ctr))
            .expect("placing a section at index 0 should succeed");

        //* When
        let nca = builder.build().expect("a small archive should build");

        //* Then
        assert_eq!(nca.header.crypto_type, 2, "the low field saturates at two");
        assert_eq!(nca.header.crypto_type2, 11);
    }

    #[test]
    fn try_from_rejects_a_key_generation_of_zero_or_past_the_last() {
        //* Given
        let out_of_range = [0u8, 33];

        //* When
        let results = out_of_range.map(KeyGeneration::try_from);

        //* Then
        assert!(
            results.iter().all(Result::is_err),
            "a generation the header cannot express must be refused"
        );
    }

    #[test]
    fn section_counter_puts_the_block_offset_in_the_low_half() {
        //* Given
        let section_ctr = [0u8; 0x8];

        //* When
        let counter = section_counter(&section_ctr, 0xC00);

        //* Then
        assert_eq!(
            &counter[..0x8],
            &[0u8; 0x8],
            "the high half mirrors the header's counter"
        );
        assert_eq!(
            u64::from_be_bytes(
                counter[0x8..]
                    .try_into()
                    .expect("an 8-byte slice converts into [u8; 8]")
            ),
            0xC0,
            "the low half counts AES blocks, not bytes"
        );
    }
}
