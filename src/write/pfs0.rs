//! PFS0 (Partition FileSystem) builder.

use std::{string::String, vec::Vec};

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
    /// Returns `BuildError::Io` if:
    /// - The directory cannot be read
    /// - A file in the directory cannot be read
    /// - A file has an invalid UTF-8 name
    pub fn from_directory(path: impl AsRef<std::path::Path>) -> Result<Self, BuildError> {
        let path = path.as_ref();
        let mut builder = Self::new();

        // Read directory entries
        let entries = std::fs::read_dir(path).map_err(|err| BuildError::Io {
            context: format!("failed to read directory '{}'", path.display()),
            source: err,
        })?;

        // Collect file entries
        let mut file_entries = Vec::new();
        for entry_result in entries {
            let entry = entry_result.map_err(|err| BuildError::Io {
                context: "failed to read directory entry".to_string(),
                source: err,
            })?;

            let entry_path = entry.path();
            let metadata = entry.metadata().map_err(|err| BuildError::Io {
                context: format!("failed to read metadata for '{}'", entry_path.display()),
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

            let name =
                entry
                    .file_name()
                    .into_string()
                    .map_err(|os_str| BuildError::InvalidFileName {
                        name: os_str.to_string_lossy().to_string(),
                        reason: "filename is not valid UTF-8",
                    })?;

            let data = std::fs::read(&entry_path).map_err(|err| BuildError::Io {
                context: format!("failed to read file '{}'", entry_path.display()),
                source: err,
            })?;

            file_entries.push((name, data));
        }

        // Sort files alphabetically by name for deterministic output
        file_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Add files to builder (validation happens in add_file)
        for (name, data) in file_entries {
            builder = builder.add_file(name, data)?;
        }

        Ok(builder)
    }

    /// Add a file to the PFS0 archive.
    ///
    /// Files are stored in the order they are added, but will be sorted
    /// alphabetically by name when the archive is built.
    pub fn add_file(
        mut self,
        name: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, BuildError> {
        let name = name.into();
        let data = data.into();

        // Validate file name
        if name.is_empty() {
            return Err(BuildError::EmptyFileName);
        }

        if name.contains('\0') {
            return Err(BuildError::InvalidFileName {
                name: name.clone(),
                reason: "filename contains null byte",
            });
        }

        if name.contains('/') || name.contains('\\') {
            return Err(BuildError::InvalidFileName {
                name: name.clone(),
                reason: "filename contains path separator",
            });
        }

        // Check for duplicates
        if self.files.iter().any(|f| f.name == name) {
            return Err(BuildError::DuplicateFile { name });
        }

        self.files.push(FileEntry { name, data });

        Ok(self)
    }

    /// Build the PFS0 archive, returning the complete binary buffer.
    ///
    /// Files are sorted alphabetically by name before being written to the archive.
    pub fn build(mut self) -> Result<Vec<u8>, BuildError> {
        // Sort files alphabetically by name (linkle parity)
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

        Ok(buf)
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

/// Error type for PFS0 builder operations.
#[derive(Debug)]
pub enum BuildError {
    /// File name is empty
    EmptyFileName,
    /// Invalid file name
    InvalidFileName { name: String, reason: &'static str },
    /// Duplicate file name
    DuplicateFile { name: String },
    /// I/O error during directory reading or file operations
    Io {
        context: String,
        source: std::io::Error,
    },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::EmptyFileName => write!(f, "file name cannot be empty"),
            BuildError::InvalidFileName { name, reason } => {
                write!(f, "invalid file name '{}': {}", name, reason)
            }
            BuildError::DuplicateFile { name } => {
                write!(f, "duplicate file name: '{}'", name)
            }
            BuildError::Io { context, .. } => write!(f, "{}", context),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
