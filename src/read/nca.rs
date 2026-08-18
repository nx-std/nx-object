//! Validated reader over a decrypted NCA and the sections it carries.
//!
//! **The buffer this reader takes is plaintext.** An NCA on disk has its header wrapped in AES-XTS
//! and its sections usually in AES-CTR, and neither is undone here — this crate does no crypto, so
//! the caller decrypts the image and hands over the result. A ciphertext buffer fails at the magic
//! check rather than parsing into nonsense.
//!
//! [`Nca::try_from_bytes`] proves every present section lies inside the buffer, and proves the data
//! region each section's superblock points at lies inside that section. That is why the accessors
//! below return slices without re-checking.
//!
//! A section entry and the FS header describing it are matched by index and by nothing else, so this
//! reader pairs them the same way and treats a zeroed entry as an absent section rather than as a
//! section of length zero.

use zerocopy::FromBytes as _;

use crate::raw::nca::{
    MEDIA_UNIT_SIZE, NCA_HEADER_SIZE, NCA_SECTION_COUNT, NCA3_MAGIC, NcaContentType, NcaCryptType,
    NcaFsHeader, NcaFsType, NcaHashType, NcaHeader, NcaSectionEntry, Pfs0Superblock,
    RomFsSuperblock,
};

/// A borrowed view of a decrypted NCA whose every section has already been bounds-checked.
///
/// Construction is where every check happens, so an `Nca` that exists is one whose sections lie
/// inside the buffer and whose content, hash, and encryption types are ones this crate knows.
pub struct Nca<'a> {
    bytes: &'a [u8],
    header: &'a NcaHeader,
    content_type: NcaContentType,
}

impl<'a> Nca<'a> {
    /// Validate `bytes` as a decrypted NCA and borrow it, proving every section lies inside it.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot hold the `0xC00`-byte header, if the magic is not
    /// `NCA3` — which is what a still-encrypted image looks like here — if the content type is not
    /// one the format defines, or if any present section has inverted bounds, runs past the end of
    /// the buffer, declares a hash or encryption type this crate does not know, or points its data
    /// region outside itself.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        let header = NcaHeader::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: NCA_HEADER_SIZE as usize,
                available: bytes.len(),
            })?
            .0;

        if header.magic.get() != NCA3_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        let content_type = content_type(header.content_type)?;

        for index in 0..NCA_SECTION_COUNT {
            // Indexing is bounded by the loop, and both arrays are `NCA_SECTION_COUNT` long.
            let entry = &header.section_entries[index];
            let fs_header = &header.fs_headers[index];

            let Some(range) = section_range(index, entry, bytes.len())? else {
                continue;
            };

            crypt_type(index, fs_header.crypt_type)?;
            let hash_type = hash_type(index, fs_header.hash_type)?;
            data_range(index, fs_header, hash_type, range.end - range.start)?;
        }

        Ok(Self {
            bytes,
            header,
            content_type,
        })
    }

    /// The header describing the archive's identity, key area, and sections.
    pub fn header(&self) -> &NcaHeader {
        self.header
    }

    /// What the archive holds, which is what the console decides how to mount it by.
    pub fn content_type(&self) -> NcaContentType {
        self.content_type
    }

    /// The title this archive belongs to.
    pub fn title_id(&self) -> u64 {
        self.header.title_id.get()
    }

    /// Length of the whole archive in bytes, as the header records it.
    pub fn nca_size(&self) -> u64 {
        self.header.nca_size.get()
    }

    /// The keyset generation the key area is wrapped with, as a keyset file names it.
    ///
    /// The header splits the generation across two fields with a gap in the numbering: the low one
    /// saturates at 2 and the high one carries anything beyond. Taking the larger and flooring at 1
    /// recovers the single number both halves stand for.
    pub fn key_generation(&self) -> u8 {
        self.header.crypto_type.max(self.header.crypto_type2).max(1)
    }

    /// The section at `index`, or `None` when the archive leaves that slot empty.
    ///
    /// # Panics
    ///
    /// Panics if `index` is not one of the four slots an NCA holds.
    pub fn section(&self, index: usize) -> Option<NcaSection<'a>> {
        assert!(
            index < NCA_SECTION_COUNT,
            "an NCA holds {NCA_SECTION_COUNT} sections"
        );

        let entry = &self.header.section_entries[index];
        let fs_header = &self.header.fs_headers[index];

        // Every value reached below was proven by `try_from_bytes`: the range lies inside the
        // buffer, the two type fields are known, and the data region lies inside the section.
        let range = section_range(index, entry, self.bytes.len()).ok()??;
        let bytes = &self.bytes[range.clone()];

        let hash_type = hash_type(index, fs_header.hash_type).ok()?;
        let data = data_range(index, fs_header, hash_type, bytes.len()).ok()?;

        Some(NcaSection {
            index,
            range,
            fs_header,
            bytes,
            data: &bytes[data],
        })
    }

    /// Every section the archive carries, skipping the slots it leaves empty.
    pub fn sections(&self) -> impl Iterator<Item = NcaSection<'a>> + '_ {
        (0..NCA_SECTION_COUNT).filter_map(|index| self.section(index))
    }
}

/// One section: where it sits, what filesystem it holds, and how it is verified and encrypted.
pub struct NcaSection<'a> {
    index: usize,
    range: core::ops::Range<usize>,
    fs_header: &'a NcaFsHeader,
    bytes: &'a [u8],
    data: &'a [u8],
}

impl<'a> NcaSection<'a> {
    /// Which of the four slots this section occupies.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Where the section sits in the image, as a byte range.
    ///
    /// This is what a caller decrypting the image in place indexes with, which is why it is offered
    /// alongside the borrowed bytes rather than left to be recomputed from the header.
    pub fn range(&self) -> core::ops::Range<usize> {
        self.range.clone()
    }

    /// The FS header describing this section.
    pub fn fs_header(&self) -> &'a NcaFsHeader {
        self.fs_header
    }

    /// The whole section, verification structures included.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The filesystem image alone, with the hash table or tree that covers it stripped.
    ///
    /// For a PFS0 section this is the archive; for a RomFS section it is the bottom level of the
    /// IVFC tree, which is the RomFS image itself.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// What kind of filesystem the section holds.
    ///
    /// A section whose `fs_type` is neither of the two the format defines reads as a RomFS, which
    /// is what the field's zero value means; the hash type is the field that decides how the
    /// superblock is read, and it was proven at construction.
    pub fn fs_type(&self) -> NcaFsType {
        match self.fs_header.fs_type {
            value if value == NcaFsType::Pfs0 as u8 => NcaFsType::Pfs0,
            _ => NcaFsType::RomFs,
        }
    }

    /// How the section's bytes are encrypted on disk.
    pub fn encryption(&self) -> NcaCryptType {
        // Proven at construction, so the fallback is unreachable rather than a default.
        crypt_type(self.index, self.fs_header.crypt_type).unwrap_or(NcaCryptType::None)
    }

    /// The verification structure covering the section, read according to its hash type.
    pub fn superblock(&self) -> Superblock<'a> {
        // Proven at construction: the hash type is one of the two, and the superblock span is a
        // fixed `0x138` bytes that both structures exactly fill.
        match hash_type(self.index, self.fs_header.hash_type) {
            Ok(NcaHashType::Pfs0) => Pfs0Superblock::ref_from_bytes(&self.fs_header.superblock)
                .map_or(Superblock::Unreadable, Superblock::Pfs0),
            _ => RomFsSuperblock::ref_from_bytes(&self.fs_header.superblock)
                .map_or(Superblock::Unreadable, Superblock::RomFs),
        }
    }

    /// The AES-CTR counter for the first byte of the section.
    ///
    /// The counter is tied to where the section sits in the file, so the same bytes at a different
    /// offset decrypt differently. A section that is not CTR-encrypted has no counter, so this
    /// returns `None` for it.
    pub fn counter(&self) -> Option<[u8; 0x10]> {
        if self.encryption() != NcaCryptType::Ctr {
            return None;
        }

        let offset = self.range.start as u64;

        let mut counter = [0u8; 0x10];
        for (index, byte) in self.fs_header.section_ctr.iter().rev().enumerate() {
            counter[index] = *byte;
        }
        counter[0x8..].copy_from_slice(&(offset / 0x10).to_be_bytes());
        Some(counter)
    }
}

/// The verification structure a section's superblock holds.
///
/// Which of the two it is comes from the FS header's hash type, and nothing in the layout
/// distinguishes them — the same `0x138` bytes read as either.
pub enum Superblock<'a> {
    /// The single-level hash table covering a PFS0 section.
    Pfs0(&'a Pfs0Superblock),
    /// The IVFC hash tree covering a RomFS section.
    RomFs(&'a RomFsSuperblock),
    /// The superblock span could not be read as either structure.
    ///
    /// Both structures are exactly the `0x138` bytes the span reserves, so this is unreachable for
    /// a section [`Nca::try_from_bytes`] accepted; it exists so that reading a superblock stays
    /// infallible rather than returning a `Result` no caller can act on.
    Unreadable,
}

/// The content type `value` names, or an error naming what was found instead.
fn content_type(value: u8) -> Result<NcaContentType, FromBytesError> {
    match value {
        value if value == NcaContentType::Program as u8 => Ok(NcaContentType::Program),
        value if value == NcaContentType::Meta as u8 => Ok(NcaContentType::Meta),
        value if value == NcaContentType::Control as u8 => Ok(NcaContentType::Control),
        value if value == NcaContentType::Manual as u8 => Ok(NcaContentType::Manual),
        value if value == NcaContentType::Data as u8 => Ok(NcaContentType::Data),
        value if value == NcaContentType::PublicData as u8 => Ok(NcaContentType::PublicData),
        found => Err(FromBytesError::UnknownContentType { found }),
    }
}

/// The hash type `value` names, or an error naming the section that carried it.
fn hash_type(index: usize, value: u8) -> Result<NcaHashType, FromBytesError> {
    match value {
        value if value == NcaHashType::Pfs0 as u8 => Ok(NcaHashType::Pfs0),
        value if value == NcaHashType::RomFs as u8 => Ok(NcaHashType::RomFs),
        found => Err(FromBytesError::UnknownHashType {
            section_index: index,
            found,
        }),
    }
}

/// The encryption type `value` names, or an error naming the section that carried it.
fn crypt_type(index: usize, value: u8) -> Result<NcaCryptType, FromBytesError> {
    match value {
        value if value == NcaCryptType::None as u8 => Ok(NcaCryptType::None),
        value if value == NcaCryptType::Xts as u8 => Ok(NcaCryptType::Xts),
        value if value == NcaCryptType::Ctr as u8 => Ok(NcaCryptType::Ctr),
        value if value == NcaCryptType::Bktr as u8 => Ok(NcaCryptType::Bktr),
        found => Err(FromBytesError::UnknownCryptType {
            section_index: index,
            found,
        }),
    }
}

/// Where one section sits in the image, or `None` when the entry is empty.
///
/// An entry that ends at media unit zero is an unused slot rather than a section of length zero,
/// which is the distinction the format leaves to the reader.
fn section_range(
    index: usize,
    entry: &NcaSectionEntry,
    available: usize,
) -> Result<Option<core::ops::Range<usize>>, FromBytesError> {
    let start = u64::from(entry.media_start_offset.get()) * MEDIA_UNIT_SIZE;
    let end = u64::from(entry.media_end_offset.get()) * MEDIA_UNIT_SIZE;

    if end == 0 {
        return Ok(None);
    }

    if end < start || start < NCA_HEADER_SIZE {
        return Err(FromBytesError::SectionBoundsInvalid {
            section_index: index,
            start,
            end,
        });
    }

    if end > available as u64 {
        return Err(FromBytesError::SectionOutOfBounds {
            section_index: index,
            start,
            end,
            available,
        });
    }

    // Both bounds were just proven to fit an in-memory buffer, so the casts cannot truncate.
    Ok(Some(start as usize..end as usize))
}

/// Where the filesystem image sits within a section of `section_size` bytes.
fn data_range(
    index: usize,
    fs_header: &NcaFsHeader,
    hash_type: NcaHashType,
    section_size: usize,
) -> Result<core::ops::Range<usize>, FromBytesError> {
    let (offset, size) = match hash_type {
        NcaHashType::Pfs0 => {
            let superblock =
                Pfs0Superblock::ref_from_bytes(&fs_header.superblock).map_err(|_| {
                    FromBytesError::SuperblockUnreadable {
                        section_index: index,
                    }
                })?;
            (superblock.pfs0_offset.get(), superblock.pfs0_size.get())
        }
        NcaHashType::RomFs => {
            let superblock =
                RomFsSuperblock::ref_from_bytes(&fs_header.superblock).map_err(|_| {
                    FromBytesError::SuperblockUnreadable {
                        section_index: index,
                    }
                })?;

            // `level_count` counts the master hash as well as the stored levels, so the bottom
            // level — the RomFS image itself — is two below it.
            let ivfc = &superblock.ivfc_header;
            let last = (ivfc.level_count.get() as usize).checked_sub(2).ok_or(
                FromBytesError::IvfcLevelCountInvalid {
                    section_index: index,
                    level_count: ivfc.level_count.get(),
                },
            )?;
            let level =
                ivfc.level_headers
                    .get(last)
                    .ok_or(FromBytesError::IvfcLevelCountInvalid {
                        section_index: index,
                        level_count: ivfc.level_count.get(),
                    })?;
            (level.logical_offset.get(), level.hash_data_size.get())
        }
    };

    let end = offset
        .checked_add(size)
        .ok_or(FromBytesError::DataRegionOutOfBounds {
            section_index: index,
            offset,
            size,
            available: section_size,
        })?;

    if end > section_size as u64 {
        return Err(FromBytesError::DataRegionOutOfBounds {
            section_index: index,
            offset,
            size,
            available: section_size,
        });
    }

    // Both bounds were just proven to fit the section, which is itself in memory.
    Ok(offset as usize..end as usize)
}

/// Errors that can occur when parsing a decrypted NCA from bytes
#[derive(Debug, thiserror::Error)]
pub enum FromBytesError {
    /// Buffer is too small to contain the NCA header
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// Magic number does not match NCA3 (0x3341434e)
    ///
    /// An image whose header is still encrypted fails here, because ciphertext does not carry the
    /// magic at the offset the format puts it.
    #[error("invalid NCA magic: found {found:#010x}; is the header still encrypted?")]
    InvalidMagic {
        /// The magic that was found in place of `NCA3`
        found: u32,
    },
    /// The content type is not one the format defines
    #[error("unknown content type {found}")]
    UnknownContentType {
        /// The value the header carried
        found: u8,
    },
    /// A section's hash type is neither of the two the format defines
    #[error("section {section_index} declares unknown hash type {found}")]
    UnknownHashType {
        /// Index of the section in the header
        section_index: usize,
        /// The value the FS header carried
        found: u8,
    },
    /// A section's encryption type is not one the format defines
    #[error("section {section_index} declares unknown encryption type {found}")]
    UnknownCryptType {
        /// Index of the section in the header
        section_index: usize,
        /// The value the FS header carried
        found: u8,
    },
    /// A section ends before it starts, or starts inside the header
    #[error("section {section_index} spans {start}..{end}, which is not a valid extent")]
    SectionBoundsInvalid {
        /// Index of the section in the header
        section_index: usize,
        /// First byte of the section
        start: u64,
        /// One past the last byte of the section
        end: u64,
    },
    /// A section extends past the end of the image
    #[error("section {section_index} ends at {end}, past the {available} available")]
    SectionOutOfBounds {
        /// Index of the section in the header
        section_index: usize,
        /// First byte of the section
        start: u64,
        /// One past the last byte of the section
        end: u64,
        /// Number of bytes the image holds
        available: usize,
    },
    /// A section's superblock span could not be read as the structure its hash type names
    #[error("section {section_index} has an unreadable superblock")]
    SuperblockUnreadable {
        /// Index of the section in the header
        section_index: usize,
    },
    /// An IVFC header declares a level count that names no bottom level
    #[error("section {section_index} declares {level_count} IVFC levels")]
    IvfcLevelCountInvalid {
        /// Index of the section in the header
        section_index: usize,
        /// The count the IVFC header carried
        level_count: u32,
    },
    /// A section's filesystem image falls outside the section
    #[error(
        "section {section_index} places its data at {offset} of size {size}, \
         past the {available} the section holds"
    )]
    DataRegionOutOfBounds {
        /// Index of the section in the header
        section_index: usize,
        /// Offset of the data region from the start of the section
        offset: u64,
        /// Length of the data region
        size: u64,
        /// Number of bytes the section holds
        available: usize,
    },
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use alloc::vec;

    use super::{FromBytesError, Nca, Superblock};
    use crate::{
        raw::nca::{NcaContentType, NcaCryptType},
        write::{
            NcaBuilder,
            nca::{KeyGeneration, Section, SectionData, SectionEncryption},
        },
    };

    /// Build a plaintext NCA holding one partition section of `size` bytes.
    fn plain_nca(size: usize) -> alloc::vec::Vec<u8> {
        NcaBuilder::new(NcaContentType::Program, 0x0100_0000_0000_1000)
            .section(
                0,
                Section {
                    data: SectionData::Partition {
                        archive: vec![0xAB; size],
                        hash_block_size: 0x1000,
                    },
                    encryption: SectionEncryption::None,
                },
            )
            .expect("placing a section at index 0 should succeed")
            .build()
            .expect("a small archive should build")
            .to_bytes()
    }

    #[test]
    fn try_from_bytes_with_a_plaintext_archive_succeeds() {
        //* Given
        let image = plain_nca(0x400);

        //* When
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");

        //* Then
        assert_eq!(nca.content_type(), NcaContentType::Program);
        assert_eq!(nca.title_id(), 0x0100_0000_0000_1000);
    }

    #[test]
    fn try_from_bytes_with_an_encrypted_header_fails() {
        //* Given
        // Ciphertext does not carry the magic where the format puts it, which is the one signal
        // this reader has that the caller skipped decryption.
        let mut image = plain_nca(0x400);
        image[0x200..0x204].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        //* When
        let result = Nca::try_from_bytes(&image);

        //* Then
        assert!(matches!(result, Err(FromBytesError::InvalidMagic { .. })));
    }

    #[test]
    fn try_from_bytes_with_a_truncated_image_fails() {
        //* Given
        let image = plain_nca(0x400);
        let truncated = &image[..0x800];

        //* When
        let result = Nca::try_from_bytes(truncated);

        //* Then
        assert!(matches!(result, Err(FromBytesError::BufferTooSmall { .. })));
    }

    #[test]
    fn try_from_bytes_with_a_section_past_the_end_fails() {
        //* Given
        // The section's end is pushed past what the buffer holds.
        let mut image = plain_nca(0x400);
        let end_at = 0x200 + 0x40 + 0x4;
        image[end_at..end_at + 4].copy_from_slice(&0xFFFFu32.to_le_bytes());

        //* When
        let result = Nca::try_from_bytes(&image);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::SectionOutOfBounds { .. })
        ));
    }

    #[test]
    fn try_from_bytes_with_an_unknown_content_type_fails() {
        //* Given
        let mut image = plain_nca(0x400);
        image[0x205] = 0x63;

        //* When
        let result = Nca::try_from_bytes(&image);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::UnknownContentType { found: 0x63 })
        ));
    }

    #[test]
    fn section_with_a_partition_section_returns_the_archive_alone() {
        //* Given
        let image = plain_nca(0x400);
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");

        //* When
        let section = nca.section(0).expect("index 0 holds a section");

        //* Then
        assert_eq!(
            section.data(),
            &vec![0xABu8; 0x400][..],
            "the hash table should be stripped from the data region"
        );
        assert!(matches!(section.superblock(), Superblock::Pfs0(_)));
        assert_eq!(section.encryption(), NcaCryptType::None);
    }

    #[test]
    fn section_with_an_empty_slot_returns_none() {
        //* Given
        let image = plain_nca(0x400);
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");

        //* When
        let section = nca.section(1);

        //* Then
        assert!(section.is_none());
    }

    #[test]
    fn sections_with_one_section_placed_yields_only_it() {
        //* Given
        let image = plain_nca(0x400);
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");

        //* When
        let indices: alloc::vec::Vec<usize> = nca.sections().map(|s| s.index()).collect();

        //* Then
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn counter_with_an_unencrypted_section_returns_none() {
        //* Given
        let image = plain_nca(0x400);
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");
        let section = nca.section(0).expect("index 0 holds a section");

        //* When
        let counter = section.counter();

        //* Then
        assert!(counter.is_none());
    }

    #[test]
    fn key_generation_with_a_first_generation_archive_returns_one() {
        //* Given
        let image = NcaBuilder::new(NcaContentType::Program, 0x0100_0000_0000_1000)
            .key_generation(KeyGeneration::FIRST)
            .section(
                0,
                Section {
                    data: SectionData::Partition {
                        archive: vec![0xAB; 0x200],
                        hash_block_size: 0x1000,
                    },
                    encryption: SectionEncryption::None,
                },
            )
            .expect("placing a section at index 0 should succeed")
            .build()
            .expect("a small archive should build")
            .to_bytes();
        let nca = Nca::try_from_bytes(&image).expect("a built archive should parse");

        //* When
        let generation = nca.key_generation();

        //* Then
        assert_eq!(
            generation, 1,
            "both header fields are zero for the first generation"
        );
    }
}
