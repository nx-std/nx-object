//! PFS0 (Partition FileSystem) builder.

use std::{path::PathBuf, string::String, vec::Vec};

use zerocopy::IntoBytes;

use crate::raw::pfs0::{PFS0_MAGIC, Pfs0FileEntry, Pfs0Header};

/// Internal file entry for building PFS0.
struct FileEntry {
    name: String,
    data: Vec<u8>,
}

/// Builder for constructing PFS0 filesystem images.
pub struct Pfs0Builder {
    files: Vec<FileEntry>,
}

impl Pfs0Builder {
    /// Create a new PFS0 builder.
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    /// Create a PFS0 archive from a directory.
    ///
    /// Reads all regular files in the directory (non-recursively). Subdirectories
    /// are skipped. Files are sorted alphabetically by name for deterministic output.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or one of its files cannot be read, if a
    /// name is not valid UTF-8, or if a scanned file is rejected by
    /// [`Pfs0Builder::add_file`]. Subdirectories are skipped rather than refused,
    /// because PFS0 has no representation for nesting.
    pub fn from_directory(path: impl AsRef<std::path::Path>) -> Result<Self, FromDirectoryError> {
        let path = path.as_ref();
        let mut builder = Self::new();

        // Read directory entries
        let entries = std::fs::read_dir(path).map_err(|err| FromDirectoryError::Io {
            path: path.to_path_buf(),
            source: err,
        })?;

        // Collect file entries
        let mut file_entries = Vec::new();
        for entry_result in entries {
            let entry = entry_result.map_err(|err| FromDirectoryError::Io {
                path: path.to_path_buf(),
                source: err,
            })?;

            let entry_path = entry.path();
            let metadata = entry.metadata().map_err(|err| FromDirectoryError::Io {
                path: entry_path.clone(),
                source: err,
            })?;

            // Skip directories
            if metadata.is_dir() {
                continue;
            }

            // Only process regular files
            if !metadata.is_file() {
                continue;
            }

            let name = entry.file_name().into_string().map_err(|_| {
                FromDirectoryError::InvalidFileName {
                    path: entry_path.clone(),
                }
            })?;

            let data = std::fs::read(&entry_path).map_err(|err| FromDirectoryError::Io {
                path: entry_path.clone(),
                source: err,
            })?;

            file_entries.push((name, data));
        }

        // Sort files alphabetically by name for deterministic output
        file_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Add files to builder (validation happens in add_file)
        for (name, data) in file_entries {
            builder = builder
                .add_file(name, data)
                .map_err(FromDirectoryError::AddFile)?;
        }

        Ok(builder)
    }

    /// Add a file to the PFS0 archive.
    ///
    /// Files are stored in the order they are added, but will be sorted
    /// alphabetically by name when the archive is built.
    ///
    /// # Errors
    ///
    /// Returns an error if the name is empty, carries a null byte or a path
    /// separator, or duplicates one already added. Every name is checked here, so
    /// [`Pfs0Builder::build`] cannot fail.
    pub fn add_file(
        mut self,
        name: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, AddFileError> {
        let name = name.into();
        let data = data.into();

        // Validate file name
        if name.is_empty() {
            return Err(AddFileError::EmptyFileName);
        }

        if name.contains('\0') {
            return Err(AddFileError::InvalidFileName {
                name: name.clone(),
                reason: "filename contains null byte",
            });
        }

        if name.contains('/') || name.contains('\\') {
            return Err(AddFileError::InvalidFileName {
                name: name.clone(),
                reason: "filename contains path separator",
            });
        }

        // Check for duplicates
        if self.files.iter().any(|f| f.name == name) {
            return Err(AddFileError::DuplicateFile { name });
        }

        self.files.push(FileEntry { name, data });

        Ok(self)
    }

    /// Build the PFS0 archive, returning the complete binary buffer.
    ///
    /// Files are sorted alphabetically by name before being written to the archive.
    ///
    /// Infallible: every name was checked by [`Pfs0Builder::add_file`], and an
    /// archive with no files is a valid, empty one.
    pub fn build(mut self) -> Vec<u8> {
        // Sort files alphabetically by name
        self.files.sort_by(|a, b| a.name.cmp(&b.name));

        let file_count = self.files.len() as u32;

        // Calculate layout offsets
        // Header: 0x10 bytes
        // File table: file_count * 0x18 bytes
        // String table: sum of (name.len() + 1) for each file, aligned to 0x20
        // Data: file data concatenated
        let file_table_size = file_count as u64 * 0x18;
        let string_table_size_raw: usize = self.files.iter().map(|f| f.name.len() + 1).sum();
        let string_table_size = align_up(string_table_size_raw, 0x20);

        let header_end = 0x10u64;
        let file_table_end = header_end + file_table_size;
        let string_table_end = file_table_end + string_table_size as u64;
        let data_start = string_table_end;

        // Calculate total size
        let total_data_size: u64 = self.files.iter().map(|f| f.data.len() as u64).sum();
        let total_size = data_start + total_data_size;

        let mut buf = vec![0u8; total_size as usize];

        // Write header
        let header = Pfs0Header {
            magic: PFS0_MAGIC.into(),
            file_count: file_count.into(),
            string_table_size: (string_table_size as u32).into(),
            _reserved: 0.into(),
        };
        buf[0..0x10].copy_from_slice(header.as_bytes());

        // Write file entries, string table, and data
        let mut string_offset = 0u32;
        let mut data_offset = 0u64;

        for (i, file) in self.files.iter().enumerate() {
            let file_entry_offset = 0x10 + i * 0x18;

            // Write file entry
            let entry = Pfs0FileEntry {
                offset: data_offset.into(),
                size: (file.data.len() as u64).into(),
                string_table_offset: string_offset.into(),
                _reserved: 0.into(),
            };
            buf[file_entry_offset..file_entry_offset + 0x18].copy_from_slice(entry.as_bytes());

            // Write filename to string table
            let name_bytes = file.name.as_bytes();
            let string_table_offset_abs = (file_table_end as usize) + (string_offset as usize);
            buf[string_table_offset_abs..string_table_offset_abs + name_bytes.len()]
                .copy_from_slice(name_bytes);
            // Null terminator already present (buffer is zero-initialized)

            // Write file data
            let data_offset_abs = data_start as usize + data_offset as usize;
            buf[data_offset_abs..data_offset_abs + file.data.len()].copy_from_slice(&file.data);

            string_offset += (name_bytes.len() + 1) as u32;
            data_offset += file.data.len() as u64;
        }

        buf
    }
}

impl Default for Pfs0Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Align value up to the nearest multiple of alignment.
#[inline]
fn align_up(value: usize, alignment: usize) -> usize {
    (value + (alignment - 1)) & !(alignment - 1)
}

/// Error returned by [`Pfs0Builder::add_file`].
#[derive(Debug, thiserror::Error)]
pub enum AddFileError {
    /// The file name is empty.
    ///
    /// PFS0 addresses a file only by its name, so an unnamed entry cannot be
    /// referred to once the archive is written.
    #[error("file name cannot be empty")]
    EmptyFileName,
    /// The file name cannot be stored in the string table.
    ///
    /// Names are null-terminated and carry no path structure, so an embedded null
    /// byte or path separator is rejected. Holds the offending name and which of
    /// the two rules it broke.
    #[error("invalid file name '{name}': {reason}")]
    InvalidFileName {
        /// The rejected name.
        name: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// Two entries resolve to the same name.
    ///
    /// Holds the duplicated name.
    #[error("duplicate file name: '{name}'")]
    DuplicateFile {
        /// The duplicated name.
        name: String,
    },
}

/// Error returned by [`Pfs0Builder::from_directory`].
#[derive(Debug, thiserror::Error)]
pub enum FromDirectoryError {
    /// A filesystem entry could not be read while scanning the directory.
    ///
    /// Holds the path being read and the underlying [`std::io::Error`].
    #[error("I/O error reading {}", path.display())]
    Io {
        /// Path that was being read when the I/O error occurred.
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A file name is not valid UTF-8.
    ///
    /// PFS0 stores names as UTF-8; an entry whose name cannot be decoded is
    /// rejected. Holds the offending path.
    #[error("invalid file name: {}", path.display())]
    InvalidFileName {
        /// Path whose name could not be decoded.
        path: PathBuf,
    },
    /// A scanned file was rejected by [`Pfs0Builder::add_file`].
    #[error("failed to add a scanned file to the archive")]
    AddFile(#[source] AddFileError),
}

#[cfg(test)]
mod tests {
    use super::{AddFileError, Pfs0Builder};

    /// Read the little-endian `u32` at `offset`.
    fn read_u32(image: &[u8], offset: usize) -> u32 {
        let bytes = &image[offset..offset + 4];
        u32::from_le_bytes(
            bytes
                .try_into()
                .expect("a 4-byte slice converts into [u8; 4]"),
        )
    }

    /// Read the little-endian `u64` at `offset`.
    fn read_u64(image: &[u8], offset: usize) -> u64 {
        let bytes = &image[offset..offset + 8];
        u64::from_le_bytes(
            bytes
                .try_into()
                .expect("an 8-byte slice converts into [u8; 8]"),
        )
    }

    #[test]
    fn build_with_no_files_produces_a_header_only_archive() {
        //* Given
        let builder = Pfs0Builder::new();

        //* When
        let image = builder.build();

        //* Then
        assert_eq!(image.len(), 0x10, "an empty archive is the header alone");
        assert_eq!(read_u32(&image, 4), 0, "file count should be zero");
    }

    #[test]
    fn build_sorts_entries_by_name_regardless_of_insertion_order() {
        //* Given
        // Added in reverse so a build that preserved insertion order would fail.
        let builder = Pfs0Builder::new()
            .add_file("main.npdm", b"npdm".to_vec())
            .expect("adding main.npdm should succeed")
            .add_file("main", b"nso".to_vec())
            .expect("adding main should succeed");

        //* When
        let image = builder.build();

        //* Then
        // The first entry's name sits at the start of the string table, which
        // follows the header and the two 0x18-byte entries.
        let string_table = 0x10 + 2 * 0x18;
        assert_eq!(
            &image[string_table..string_table + 5],
            b"main\0",
            "the alphabetically first name should head the string table"
        );
        assert_eq!(
            read_u64(&image, 0x10 + 8),
            3,
            "the first entry should be `main`, whose data is 3 bytes"
        );
    }

    #[test]
    fn build_pads_the_string_table_to_its_alignment() {
        //* Given
        let builder = Pfs0Builder::new()
            .add_file("a", b"x".to_vec())
            .expect("adding a should succeed");

        //* When
        let image = builder.build();

        //* Then
        // "a\0" is 2 bytes, padded up to the 0x20 boundary the format requires.
        assert_eq!(
            read_u32(&image, 8),
            0x20,
            "the recorded string table size should be the padded one"
        );
        assert_eq!(
            image.len(),
            0x10 + 0x18 + 0x20 + 1,
            "header, one entry, padded string table, then the file data"
        );
    }

    #[test]
    fn add_file_with_an_empty_name_fails() {
        //* Given
        let builder = Pfs0Builder::new();

        //* When
        let result = builder.add_file("", b"data".to_vec());

        //* Then
        assert!(
            matches!(result, Err(AddFileError::EmptyFileName)),
            "an unnamed entry could not be addressed once written"
        );
    }

    #[test]
    fn add_file_with_a_path_separator_fails() {
        //* Given
        let builder = Pfs0Builder::new();

        //* When
        let result = builder.add_file("dir/file", b"data".to_vec());

        //* Then
        assert!(
            matches!(result, Err(AddFileError::InvalidFileName { .. })),
            "PFS0 names carry no path structure, so a separator must be refused"
        );
    }

    #[test]
    fn add_file_with_a_duplicate_name_fails() {
        //* Given
        let builder = Pfs0Builder::new()
            .add_file("main", b"first".to_vec())
            .expect("adding main should succeed");

        //* When
        let result = builder.add_file("main", b"second".to_vec());

        //* Then
        assert!(
            matches!(result, Err(AddFileError::DuplicateFile { .. })),
            "two entries with one name cannot both be addressed"
        );
    }
}
