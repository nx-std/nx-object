//! Builder for the content meta file that names every NCA of a title.
//!
//! A CNMT is written once all the other archives exist, because each record carries the SHA-256 of a
//! finished NCA and that hash is also the archive's name. The builder therefore takes hashes and
//! sizes rather than the archives themselves: by the time it runs, the bytes it describes have
//! already been written.
//!
//! Records are sorted by content type on build. The format fixes no order, so sorting is what makes
//! the same set of contents produce the same bytes.

use alloc::vec::Vec;

use zerocopy::IntoBytes as _;

use crate::raw::cnmt::{
    CnmtApplicationExtendedHeader, CnmtContentMetaType, CnmtContentRecord, CnmtContentType,
    CnmtHeader,
};

/// Distance from a title ID to the ID its updates are published under.
const PATCH_TITLE_ID_OFFSET: u64 = 0x800;

/// Largest content size the six-byte size field can hold.
const MAX_CONTENT_SIZE: u64 = (1 << 48) - 1;

/// One finished NCA, as the content meta records it.
#[derive(Debug, Clone, Copy)]
pub struct ContentRecord {
    /// SHA-256 of the whole NCA file. Its first sixteen bytes are also the archive's name.
    pub hash: [u8; 0x20],
    /// Length of the NCA in bytes.
    pub size: u64,
    /// What the NCA holds.
    pub content_type: CnmtContentType,
}

/// Accumulates content records and lays them out as an application's content meta.
///
/// Only the application meta type is produced. The patch title ID in the extended header is derived
/// from the title ID rather than supplied, because the console derives it the same way.
pub struct CnmtBuilder {
    title_id: u64,
    title_version: u32,
    records: Vec<ContentRecord>,
}

impl CnmtBuilder {
    /// Start a content meta for `title_id` at version zero, describing nothing yet.
    pub fn new(title_id: u64) -> Self {
        Self {
            title_id,
            title_version: 0,
            records: Vec::new(),
        }
    }

    /// Set the title version the meta describes.
    pub fn title_version(mut self, title_version: u32) -> Self {
        self.title_version = title_version;
        self
    }

    /// Record one finished NCA.
    ///
    /// # Errors
    ///
    /// Returns an error if the NCA is larger than the six-byte size field can hold, or if a record
    /// of the same content type was already added. Every record is checked here, so
    /// [`CnmtBuilder::build`] cannot fail.
    pub fn add_content(mut self, record: ContentRecord) -> Result<Self, AddContentError> {
        if record.size > MAX_CONTENT_SIZE {
            return Err(AddContentError::ContentTooLarge { size: record.size });
        }

        if self
            .records
            .iter()
            .any(|existing| existing.content_type as u8 == record.content_type as u8)
        {
            return Err(AddContentError::DuplicateContentType {
                content_type: record.content_type,
            });
        }

        self.records.push(record);

        Ok(self)
    }

    /// The name the meta file must be stored under inside its archive.
    ///
    /// The console locates the content meta by name, so the file added to the meta NCA's PFS0 has to
    /// carry exactly this one.
    pub fn file_name(&self) -> alloc::string::String {
        alloc::format!("Application_{:016x}.cnmt", self.title_id)
    }

    /// Sort the records, lay out the header and tables, and return the finished meta file.
    ///
    /// Infallible: every record was checked by [`CnmtBuilder::add_content`], and a meta describing
    /// no contents is a valid, if useless, one.
    pub fn build(mut self) -> Vec<u8> {
        self.records.sort_by_key(|record| record.content_type as u8);

        let header = CnmtHeader {
            title_id: self.title_id.into(),
            title_version: self.title_version.into(),
            meta_type: CnmtContentMetaType::Application as u8,
            _reserved_0xd: 0,
            // Both narrowings are bounded by construction: the extended header is a fixed 0x10-byte
            // structure, and `add_content` admits one record per content type, of which there are
            // seven.
            extended_header_size: (size_of::<CnmtApplicationExtendedHeader>() as u16).into(),
            content_entry_count: (self.records.len() as u16).into(),
            meta_entry_count: 0.into(),
            _reserved_0x14: [0; 0xC],
        };

        let extended_header = CnmtApplicationExtendedHeader {
            patch_title_id: self.title_id.wrapping_add(PATCH_TITLE_ID_OFFSET).into(),
            required_system_version: 0.into(),
            required_application_version: 0.into(),
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(extended_header.as_bytes());

        for record in &self.records {
            let mut nca_id = [0u8; 0x10];
            nca_id.copy_from_slice(&record.hash[..0x10]);

            let mut size = [0u8; 0x6];
            size.copy_from_slice(&record.size.to_le_bytes()[..0x6]);

            let raw = CnmtContentRecord {
                hash: record.hash,
                nca_id,
                size,
                content_type: record.content_type as u8,
                id_offset: 0,
            };
            buf.extend_from_slice(raw.as_bytes());
        }

        // The trailing digest belongs to the signed distribution path, which a title installed
        // from an NSP does not take, so the field is present and left zero.
        buf.extend_from_slice(&[0u8; 0x20]);

        buf
    }
}

/// Error returned by [`CnmtBuilder::add_content`].
#[derive(Debug, thiserror::Error)]
pub enum AddContentError {
    /// The NCA is larger than the six-byte size field can record.
    ///
    /// Holds the size that could not be stored.
    #[error("content of {size} bytes exceeds the six-byte size field")]
    ContentTooLarge {
        /// The size that could not be stored.
        size: u64,
    },
    /// A record of this content type was already added.
    ///
    /// Every content type appears at most once in a homebrew title's meta, and a second one would be
    /// indistinguishable from the first without an `id_offset` scheme this builder does not model.
    /// Holds the duplicated type.
    #[error("content type {content_type:?} was already recorded")]
    DuplicateContentType {
        /// The duplicated type.
        content_type: CnmtContentType,
    },
}

#[cfg(test)]
mod tests {
    use super::{CnmtBuilder, ContentRecord};
    use crate::raw::cnmt::CnmtContentType;

    /// A record whose hash is `byte` repeated, so it is easy to spot in the output.
    fn record(byte: u8, content_type: CnmtContentType) -> ContentRecord {
        ContentRecord {
            hash: [byte; 0x20],
            size: 0x1000,
            content_type,
        }
    }

    #[test]
    fn build_names_the_nca_by_the_head_of_its_hash() {
        //* Given
        let builder = CnmtBuilder::new(0x0100000000001000)
            .add_content(record(0xAB, CnmtContentType::Program))
            .expect("adding a program record should succeed");

        //* When
        let meta = builder.build();

        //* Then
        // The single record follows the 0x20 header and the 0x10 extended header.
        let nca_id = &meta[0x30 + 0x20..0x30 + 0x30];
        assert_eq!(
            nca_id, &[0xAB; 0x10],
            "the NCA ID should be the first sixteen bytes of the hash"
        );
    }

    #[test]
    fn build_orders_records_by_content_type_regardless_of_insertion_order() {
        //* Given
        // Added control-first so a build preserving insertion order would fail.
        let builder = CnmtBuilder::new(0x0100000000001000)
            .add_content(record(0x02, CnmtContentType::Control))
            .expect("adding a control record should succeed")
            .add_content(record(0x01, CnmtContentType::Program))
            .expect("adding a program record should succeed");

        //* When
        let meta = builder.build();

        //* Then
        let first_record_type = meta[0x30 + 0x36];
        assert_eq!(
            first_record_type,
            CnmtContentType::Program as u8,
            "program sorts before control and should lead the records"
        );
    }

    #[test]
    fn build_derives_the_patch_title_id_from_the_title_id() {
        //* Given
        let builder = CnmtBuilder::new(0x0100000000001000);

        //* When
        let meta = builder.build();

        //* Then
        let patch_title_id = u64::from_le_bytes(
            meta[0x20..0x28]
                .try_into()
                .expect("an 8-byte slice converts into [u8; 8]"),
        );
        assert_eq!(
            patch_title_id, 0x0100000000001800,
            "updates are published 0x800 above the title ID"
        );
    }

    #[test]
    fn file_name_matches_what_the_console_looks_up() {
        //* Given
        let builder = CnmtBuilder::new(0x0100000000001000);

        //* When
        let name = builder.file_name();

        //* Then
        assert_eq!(name, "Application_0100000000001000.cnmt");
    }

    #[test]
    fn add_content_with_a_repeated_content_type_fails() {
        //* Given
        let builder = CnmtBuilder::new(0x0100000000001000)
            .add_content(record(0x01, CnmtContentType::Program))
            .expect("adding a program record should succeed");

        //* When
        let result = builder.add_content(record(0x02, CnmtContentType::Program));

        //* Then
        assert!(
            result.is_err(),
            "two program records could not be told apart by the console"
        );
    }
}
