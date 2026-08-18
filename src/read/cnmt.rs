//! Validated reader over a CNMT and the content records it lists.
//!
//! [`Cnmt::try_from_bytes`] proves the extended header and every record lie inside the buffer before
//! borrowing it, so the accessors return slices without re-checking.
//!
//! A CNMT has no magic. Success here means the buffer is shaped like one — its declared counts fit
//! its length — never that it is one, which is the same caveat NACP and RomFS carry.
//!
//! The records name every content of the title *except* the meta itself: the archive carrying this
//! CNMT is found by whatever referenced it, so listing itself would be circular. A caller checking
//! an NSP against its meta therefore expects one more file in the package than there are records.

use zerocopy::FromBytes as _;

use crate::raw::cnmt::{
    CnmtApplicationExtendedHeader, CnmtContentMetaType, CnmtContentRecord, CnmtContentType,
    CnmtHeader,
};

/// A borrowed view of a CNMT whose records have already been bounds-checked.
pub struct Cnmt<'a> {
    header: &'a CnmtHeader,
    extended_header: Option<&'a CnmtApplicationExtendedHeader>,
    records: &'a [CnmtContentRecord],
}

impl<'a> Cnmt<'a> {
    /// Validate `bytes` as a CNMT and borrow it, proving every record lies inside the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot hold the header, the extended header the header
    /// declares, or the records it counts.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        let (header, rest) =
            CnmtHeader::ref_from_prefix(bytes).map_err(|_| FromBytesError::BufferTooSmall {
                required: size_of::<CnmtHeader>(),
                available: bytes.len(),
            })?;

        let extended_header_size = header.extended_header_size.get() as usize;
        let rest =
            rest.get(extended_header_size..)
                .ok_or(FromBytesError::ExtendedHeaderOutOfBounds {
                    extended_header_size,
                    available: rest.len(),
                })?;

        // Read only when the declared size matches the application shape. A meta type this crate
        // does not model still parses — its records are what a caller came for — but its extended
        // header is left unread rather than reinterpreted as the wrong structure.
        let extended_header = if extended_header_size == size_of::<CnmtApplicationExtendedHeader>()
            && header.meta_type == CnmtContentMetaType::Application as u8
        {
            CnmtApplicationExtendedHeader::ref_from_prefix(&bytes[size_of::<CnmtHeader>()..])
                .ok()
                .map(|(extended, _)| extended)
        } else {
            None
        };

        let count = header.content_entry_count.get() as usize;
        let (records, _) =
            <[CnmtContentRecord]>::ref_from_prefix_with_elems(rest, count).map_err(|_| {
                FromBytesError::RecordsOutOfBounds {
                    content_entry_count: count,
                    available: rest.len(),
                }
            })?;

        Ok(Self {
            header,
            extended_header,
            records,
        })
    }

    /// The header naming the title and counting what follows it.
    pub fn header(&self) -> &'a CnmtHeader {
        self.header
    }

    /// The application extended header, or `None` for a meta type this crate does not model.
    pub fn extended_header(&self) -> Option<&'a CnmtApplicationExtendedHeader> {
        self.extended_header
    }

    /// The title the described contents belong to.
    pub fn title_id(&self) -> u64 {
        self.header.title_id.get()
    }

    /// The version of the title; zero for a first release.
    pub fn title_version(&self) -> u32 {
        self.header.title_version.get()
    }

    /// What kind of title this content meta describes.
    pub fn meta_type(&self) -> Option<CnmtContentMetaType> {
        meta_type(self.header.meta_type)
    }

    /// Every content record, in the order the file lists them.
    pub fn records(&self) -> impl Iterator<Item = ContentRecord<'a>> + '_ {
        self.records.iter().map(|record| ContentRecord { record })
    }

    /// Number of content records the file holds.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
}

/// One content of the title: which NCA it is, how large, and what it holds.
pub struct ContentRecord<'a> {
    record: &'a CnmtContentRecord,
}

impl<'a> ContentRecord<'a> {
    /// The raw record, for a caller that needs a field this view does not surface.
    pub fn raw(&self) -> &'a CnmtContentRecord {
        self.record
    }

    /// SHA-256 of the whole NCA file.
    pub fn hash(&self) -> &'a [u8; 0x20] {
        &self.record.hash
    }

    /// The NCA's name on disk, which is the first sixteen bytes of its hash.
    pub fn nca_id(&self) -> &'a [u8; 0x10] {
        &self.record.nca_id
    }

    /// Length of the NCA in bytes.
    ///
    /// Stored across six bytes rather than eight, so the value cannot exceed `0xFFFFFFFFFFFF`.
    pub fn size(&self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes[..0x6].copy_from_slice(&self.record.size);
        u64::from_le_bytes(bytes)
    }

    /// What the content holds.
    pub fn content_type(&self) -> Option<CnmtContentType> {
        content_type(self.record.content_type)
    }
}

/// The meta type `value` names, or `None` for one this crate does not model.
fn meta_type(value: u8) -> Option<CnmtContentMetaType> {
    match value {
        value if value == CnmtContentMetaType::Application as u8 => {
            Some(CnmtContentMetaType::Application)
        }
        value if value == CnmtContentMetaType::Patch as u8 => Some(CnmtContentMetaType::Patch),
        value if value == CnmtContentMetaType::AddOnContent as u8 => {
            Some(CnmtContentMetaType::AddOnContent)
        }
        _ => None,
    }
}

/// The content type `value` names, or `None` for one this crate does not model.
fn content_type(value: u8) -> Option<CnmtContentType> {
    match value {
        value if value == CnmtContentType::Meta as u8 => Some(CnmtContentType::Meta),
        value if value == CnmtContentType::Program as u8 => Some(CnmtContentType::Program),
        value if value == CnmtContentType::Data as u8 => Some(CnmtContentType::Data),
        value if value == CnmtContentType::Control as u8 => Some(CnmtContentType::Control),
        value if value == CnmtContentType::HtmlDocument as u8 => {
            Some(CnmtContentType::HtmlDocument)
        }
        value if value == CnmtContentType::LegalInformation as u8 => {
            Some(CnmtContentType::LegalInformation)
        }
        value if value == CnmtContentType::DeltaFragment as u8 => {
            Some(CnmtContentType::DeltaFragment)
        }
        _ => None,
    }
}

/// Errors that can occur when parsing a CNMT from bytes
#[derive(Debug, thiserror::Error)]
pub enum FromBytesError {
    /// Buffer is too small to contain the content meta header
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// The extended header the header declares does not fit in the buffer
    #[error("an extended header of {extended_header_size} bytes does not fit in {available}")]
    ExtendedHeaderOutOfBounds {
        /// Length the header declares for the extended header
        extended_header_size: usize,
        /// Number of bytes left after the header
        available: usize,
    },
    /// The records the header counts do not fit in the buffer
    #[error("{content_entry_count} content records do not fit in {available} bytes")]
    RecordsOutOfBounds {
        /// Number of records the header counts
        content_entry_count: usize,
        /// Number of bytes left after the extended header
        available: usize,
    },
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use alloc::vec::Vec;

    use super::{Cnmt, FromBytesError};
    use crate::{
        raw::cnmt::{CnmtContentMetaType, CnmtContentType},
        write::{CnmtBuilder, cnmt::ContentRecord},
    };

    /// A CNMT naming a program and a control, built the way the packer builds one.
    fn cnmt() -> Vec<u8> {
        CnmtBuilder::new(0x0100_0000_0000_1000)
            .add_content(ContentRecord {
                hash: [0xAB; 0x20],
                size: 0x1234,
                content_type: CnmtContentType::Program,
            })
            .expect("adding a program record should succeed")
            .add_content(ContentRecord {
                hash: [0xCD; 0x20],
                size: 0x5678,
                content_type: CnmtContentType::Control,
            })
            .expect("adding a control record should succeed")
            .build()
    }

    #[test]
    fn try_from_bytes_with_a_built_meta_succeeds() {
        //* Given
        let meta = cnmt();

        //* When
        let cnmt = Cnmt::try_from_bytes(&meta).expect("a built meta should parse");

        //* Then
        assert_eq!(cnmt.title_id(), 0x0100_0000_0000_1000);
        assert_eq!(cnmt.record_count(), 2);
        assert_eq!(cnmt.meta_type(), Some(CnmtContentMetaType::Application));
    }

    #[test]
    fn records_with_a_built_meta_returns_what_was_added() {
        //* Given
        let meta = cnmt();
        let cnmt = Cnmt::try_from_bytes(&meta).expect("a built meta should parse");

        //* When
        let records: Vec<(u64, Option<CnmtContentType>)> = cnmt
            .records()
            .map(|record| (record.size(), record.content_type()))
            .collect();

        //* Then
        assert_eq!(
            records,
            [
                (0x1234, Some(CnmtContentType::Program)),
                (0x5678, Some(CnmtContentType::Control)),
            ]
        );
    }

    #[test]
    fn nca_id_with_a_built_meta_is_the_head_of_the_hash() {
        //* Given
        // The console looks a content up by this name, so the two fields agreeing is the whole
        // reason a record can be resolved to a file at all.
        let meta = cnmt();
        let cnmt = Cnmt::try_from_bytes(&meta).expect("a built meta should parse");

        //* When
        let record = cnmt.records().next().expect("the meta holds two records");

        //* Then
        assert_eq!(record.nca_id(), &record.hash()[..0x10]);
    }

    #[test]
    fn extended_header_with_an_application_meta_derives_the_patch_title_id() {
        //* Given
        let meta = cnmt();
        let cnmt = Cnmt::try_from_bytes(&meta).expect("a built meta should parse");

        //* When
        let extended = cnmt.extended_header();

        //* Then
        assert_eq!(
            extended
                .expect("an application meta carries one")
                .patch_title_id
                .get(),
            0x0100_0000_0000_1800
        );
    }

    #[test]
    fn try_from_bytes_with_a_truncated_buffer_fails() {
        //* Given
        let meta = cnmt();
        let truncated = &meta[..0x10];

        //* When
        let result = Cnmt::try_from_bytes(truncated);

        //* Then
        assert!(matches!(result, Err(FromBytesError::BufferTooSmall { .. })));
    }

    #[test]
    fn try_from_bytes_with_more_records_than_the_buffer_holds_fails() {
        //* Given
        let mut meta = cnmt();
        meta[0x10..0x12].copy_from_slice(&0xFFFFu16.to_le_bytes());

        //* When
        let result = Cnmt::try_from_bytes(&meta);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::RecordsOutOfBounds { .. })
        ));
    }
}
