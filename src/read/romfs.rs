use zerocopy::FromBytes;

use crate::raw::romfs::{RomFsDirEntry, RomFsFileEntry, RomFsHeader};

/// High-level RomFS parser with filesystem navigation.
pub struct RomFs<'a> {
    bytes: &'a [u8],
    header: &'a RomFsHeader,
}

impl<'a> RomFs<'a> {
    /// Parse RomFS from bytes with header validation.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        if bytes.len() < size_of::<RomFsHeader>() {
            return Err(FromBytesError::BufferTooSmall {
                required: size_of::<RomFsHeader>(),
                available: bytes.len(),
            });
        }

        let header = RomFsHeader::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x50,
                available: bytes.len(),
            })?
            .0;

        // Validate header size
        if header.header_size.get() as usize != size_of::<RomFsHeader>() {
            return Err(FromBytesError::InvalidHeaderSize {
                found: header.header_size.get() as usize,
            });
        }

        // Validate dir_meta_table bounds
        let dir_off = header.dir_meta_table_offset.get() as usize;
        let dir_size = header.dir_meta_table_size.get() as usize;
        let dir_end =
            dir_off
                .checked_add(dir_size)
                .ok_or(FromBytesError::DirMetaTableBoundsOverflow {
                    offset: dir_off,
                    size: dir_size,
                })?;
        if dir_end > bytes.len() {
            return Err(FromBytesError::DirMetaTableOutOfBounds {
                offset: dir_off,
                size: dir_size,
                available: bytes.len(),
            });
        }

        // Validate dir_hash_table bounds
        let dir_hash_off = header.dir_hash_table_offset.get() as usize;
        let dir_hash_size = header.dir_hash_table_size.get() as usize;
        let dir_hash_end = dir_hash_off.checked_add(dir_hash_size).ok_or(
            FromBytesError::DirHashTableBoundsOverflow {
                offset: dir_hash_off,
                size: dir_hash_size,
            },
        )?;
        if dir_hash_end > bytes.len() {
            return Err(FromBytesError::DirHashTableOutOfBounds {
                offset: dir_hash_off,
                size: dir_hash_size,
                available: bytes.len(),
            });
        }

        // Validate file_meta_table bounds
        let file_off = header.file_meta_table_offset.get() as usize;
        let file_size = header.file_meta_table_size.get() as usize;
        let file_end =
            file_off
                .checked_add(file_size)
                .ok_or(FromBytesError::FileMetaTableBoundsOverflow {
                    offset: file_off,
                    size: file_size,
                })?;
        if file_end > bytes.len() {
            return Err(FromBytesError::FileMetaTableOutOfBounds {
                offset: file_off,
                size: file_size,
                available: bytes.len(),
            });
        }

        // Validate file_hash_table bounds
        let file_hash_off = header.file_hash_table_offset.get() as usize;
        let file_hash_size = header.file_hash_table_size.get() as usize;
        let file_hash_end = file_hash_off.checked_add(file_hash_size).ok_or(
            FromBytesError::FileHashTableBoundsOverflow {
                offset: file_hash_off,
                size: file_hash_size,
            },
        )?;
        if file_hash_end > bytes.len() {
            return Err(FromBytesError::FileHashTableOutOfBounds {
                offset: file_hash_off,
                size: file_hash_size,
                available: bytes.len(),
            });
        }

        // Validate file_data bounds
        let data_off = header.file_data_offset.get() as usize;
        // Note: file_data has no explicit size field; it extends to end of buffer
        // We only validate that the offset itself is within bounds
        if data_off > bytes.len() {
            return Err(FromBytesError::FileDataOffsetOutOfBounds {
                offset: data_off,
                available: bytes.len(),
            });
        }

        Ok(Self { bytes, header })
    }

    /// Get the RomFS header.
    pub fn header(&self) -> &RomFsHeader {
        self.header
    }

    /// Get root directory
    pub fn root_dir(&'a self) -> Result<RomFsDir<'a>, RootDirError> {
        // Root directory is at offset 0 in dir meta table
        let dir_table_offset = self.header.dir_meta_table_offset.get() as usize;
        RomFsDir::from_offset_for_root(self, 0, dir_table_offset)
    }

    /// Open a file by path (e.g., "/config.json")
    pub fn open(&'a self, path: &str) -> Result<RomFsFile<'a>, OpenError> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return Err(OpenError::PathNotFound);
        }

        // Start from root
        let mut current_dir = self.root_dir().map_err(|e| OpenError::BufferTooSmall {
            required: e.required,
            available: e.available,
        })?;

        // Split path and traverse
        let mut components = path.split('/');
        let mut last_component = None;

        for component in components.by_ref() {
            if let Some(prev) = last_component {
                // Traverse to directory
                current_dir = current_dir.find_dir(prev)?;
            }
            last_component = Some(component);
        }

        // Last component is the file
        if let Some(filename) = last_component {
            current_dir.find_file(filename)
        } else {
            Err(OpenError::PathNotFound)
        }
    }

    fn dir_meta_table(&self) -> &'a [u8] {
        let offset = self.header.dir_meta_table_offset.get() as usize;
        let size = self.header.dir_meta_table_size.get() as usize;
        // SAFETY: Bounds validated in try_from_bytes
        &self.bytes[offset..offset + size]
    }

    fn file_meta_table(&self) -> &'a [u8] {
        let offset = self.header.file_meta_table_offset.get() as usize;
        let size = self.header.file_meta_table_size.get() as usize;
        // SAFETY: Bounds validated in try_from_bytes
        &self.bytes[offset..offset + size]
    }

    fn file_data(&self, offset: usize, size: usize) -> &'a [u8] {
        let data_offset = self.header.file_data_offset.get() as usize;
        // SAFETY: data_offset validated in try_from_bytes; offset+size validated at call site
        &self.bytes[data_offset + offset..data_offset + offset + size]
    }
}

/// RomFS directory handle with child iteration.
pub struct RomFsDir<'a> {
    romfs: &'a RomFs<'a>,
    entry: &'a RomFsDirEntry,
    name: &'a str,
}

impl<'a> RomFsDir<'a> {
    fn from_offset_for_root(
        romfs: &'a RomFs<'a>,
        offset: u32,
        table_base: usize,
    ) -> Result<Self, RootDirError> {
        let offset = offset as usize;
        let table = romfs.dir_meta_table();

        if offset >= table.len() {
            return Err(RootDirError {
                required: offset + size_of::<RomFsDirEntry>(),
                available: table_base + table.len(),
            });
        }

        let entry_bytes = &table[offset..];
        let entry = RomFsDirEntry::ref_from_prefix(entry_bytes)
            .map_err(|_| RootDirError {
                required: offset + size_of::<RomFsDirEntry>(),
                available: table_base + table.len(),
            })?
            .0;

        // Extract name with bounds validation
        let name_len = entry.name_len.get() as usize;
        let name_offset = offset + size_of::<RomFsDirEntry>();
        let name_end = name_offset.checked_add(name_len).ok_or(RootDirError {
            required: usize::MAX,
            available: table_base + table.len(),
        })?;
        if name_end > table.len() {
            return Err(RootDirError {
                required: table_base + name_end,
                available: table_base + table.len(),
            });
        }
        let name_bytes = &table[name_offset..name_end];
        let name = core::str::from_utf8(name_bytes).unwrap_or("");

        Ok(Self { romfs, entry, name })
    }

    fn from_offset(
        romfs: &'a RomFs<'a>,
        offset: u32,
        table_base: usize,
    ) -> Result<Self, OpenError> {
        let offset = offset as usize;
        let table = romfs.dir_meta_table();

        if offset >= table.len() {
            return Err(OpenError::BufferTooSmall {
                required: offset + size_of::<RomFsDirEntry>(),
                available: table_base + table.len(),
            });
        }

        let entry_bytes = &table[offset..];
        let entry = RomFsDirEntry::ref_from_prefix(entry_bytes)
            .map_err(|_| OpenError::BufferTooSmall {
                required: offset + size_of::<RomFsDirEntry>(),
                available: table_base + table.len(),
            })?
            .0;

        // Extract name with bounds validation
        let name_len = entry.name_len.get() as usize;
        let name_offset = offset + size_of::<RomFsDirEntry>();
        let name_end = name_offset
            .checked_add(name_len)
            .ok_or(OpenError::BufferTooSmall {
                required: usize::MAX,
                available: table_base + table.len(),
            })?;
        if name_end > table.len() {
            return Err(OpenError::BufferTooSmall {
                required: table_base + name_end,
                available: table_base + table.len(),
            });
        }
        let name_bytes = &table[name_offset..name_end];
        let name = core::str::from_utf8(name_bytes).unwrap_or("");

        Ok(Self { romfs, entry, name })
    }

    /// Get the directory name.
    pub fn name(&self) -> &str {
        self.name
    }

    /// Find a child directory by name
    fn find_dir(&self, name: &str) -> Result<RomFsDir<'a>, OpenError> {
        let mut child_offset = self.entry.child_offset.get();
        let table_base = self.romfs.header.dir_meta_table_offset.get() as usize;

        while child_offset != u32::MAX {
            let child = RomFsDir::from_offset(self.romfs, child_offset, table_base)?;
            if child.name() == name {
                return Ok(child);
            }
            child_offset = child.entry.sibling_offset.get();
        }

        Err(OpenError::DirNotFound)
    }

    /// Find a child file by name
    fn find_file(&self, name: &str) -> Result<RomFsFile<'a>, OpenError> {
        let mut file_offset = self.entry.file_offset.get();
        let table_base = self.romfs.header.file_meta_table_offset.get() as usize;

        while file_offset != u32::MAX {
            let file = RomFsFile::from_offset(self.romfs, file_offset, table_base)?;
            if file.name() == name {
                return Ok(file);
            }
            file_offset = file.entry.sibling_offset.get();
        }

        Err(OpenError::FileNotFound)
    }

    /// Iterate through all entries (directories and files)
    pub fn entries(&self) -> DirIterator<'a> {
        DirIterator {
            romfs: self.romfs,
            next_dir_offset: self.entry.child_offset.get(),
            next_file_offset: self.entry.file_offset.get(),
            dir_table_base: self.romfs.header.dir_meta_table_offset.get() as usize,
            file_table_base: self.romfs.header.file_meta_table_offset.get() as usize,
        }
    }
}

/// RomFS file handle with data access.
pub struct RomFsFile<'a> {
    romfs: &'a RomFs<'a>,
    entry: &'a RomFsFileEntry,
    name: &'a str,
}

impl<'a> RomFsFile<'a> {
    fn from_offset(
        romfs: &'a RomFs<'a>,
        offset: u32,
        table_base: usize,
    ) -> Result<Self, OpenError> {
        let offset = offset as usize;
        let table = romfs.file_meta_table();

        if offset >= table.len() {
            return Err(OpenError::BufferTooSmall {
                required: offset + size_of::<RomFsFileEntry>(),
                available: table_base + table.len(),
            });
        }

        let entry_bytes = &table[offset..];
        let entry = RomFsFileEntry::ref_from_prefix(entry_bytes)
            .map_err(|_| OpenError::BufferTooSmall {
                required: offset + size_of::<RomFsFileEntry>(),
                available: table_base + table.len(),
            })?
            .0;

        // Extract name with bounds validation
        let name_len = entry.name_len.get() as usize;
        let name_offset = offset + size_of::<RomFsFileEntry>();
        let name_end = name_offset
            .checked_add(name_len)
            .ok_or(OpenError::BufferTooSmall {
                required: usize::MAX,
                available: table_base + table.len(),
            })?;
        if name_end > table.len() {
            return Err(OpenError::BufferTooSmall {
                required: table_base + name_end,
                available: table_base + table.len(),
            });
        }
        let name_bytes = &table[name_offset..name_end];
        let name = core::str::from_utf8(name_bytes).unwrap_or("");

        Ok(Self { romfs, entry, name })
    }

    /// Get the file name.
    pub fn name(&self) -> &str {
        self.name
    }

    /// Get the file size in bytes.
    pub fn size(&self) -> usize {
        self.entry.data_size.get() as usize
    }

    /// Get the file contents.
    ///
    /// Returns an error if file data offset+size is out of bounds. This indicates a malformed
    /// RomFS where the file metadata table contains invalid data offsets that were not validated
    /// during initial parsing (as they are lazily accessed during filesystem traversal).
    pub fn data(&self) -> Result<&'a [u8], FromBytesError> {
        let offset = self.entry.data_offset.get() as usize;
        let size = self.entry.data_size.get() as usize;
        let data_base = self.romfs.header.file_data_offset.get() as usize;

        // Validate file data bounds at access time (not during try_from_bytes)
        let abs_start =
            data_base
                .checked_add(offset)
                .ok_or(FromBytesError::FileDataBoundsOverflow {
                    file_data_offset: data_base,
                    data_offset: offset,
                    data_size: size,
                })?;
        let abs_end =
            abs_start
                .checked_add(size)
                .ok_or(FromBytesError::FileDataBoundsOverflow {
                    file_data_offset: data_base,
                    data_offset: offset,
                    data_size: size,
                })?;

        if abs_end > self.romfs.bytes.len() {
            return Err(FromBytesError::FileDataOutOfBounds {
                data_offset: offset,
                data_size: size,
                available: self.romfs.bytes.len(),
            });
        }

        Ok(self.romfs.file_data(offset, size))
    }
}

/// Directory entry (either a file or subdirectory).
pub enum RomFsEntry<'a> {
    /// File entry
    File(RomFsFile<'a>),
    /// Directory entry
    Dir(RomFsDir<'a>),
}

/// Iterator over directory entries (subdirectories then files).
pub struct DirIterator<'a> {
    romfs: &'a RomFs<'a>,
    next_dir_offset: u32,
    next_file_offset: u32,
    dir_table_base: usize,
    file_table_base: usize,
}

impl<'a> Iterator for DirIterator<'a> {
    type Item = RomFsEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // First iterate through directories
        if self.next_dir_offset != u32::MAX
            && let Ok(dir) =
                RomFsDir::from_offset(self.romfs, self.next_dir_offset, self.dir_table_base)
        {
            self.next_dir_offset = dir.entry.sibling_offset.get();
            return Some(RomFsEntry::Dir(dir));
        }

        // Then iterate through files
        if self.next_file_offset != u32::MAX
            && let Ok(file) =
                RomFsFile::from_offset(self.romfs, self.next_file_offset, self.file_table_base)
        {
            self.next_file_offset = file.entry.sibling_offset.get();
            return Some(RomFsEntry::File(file));
        }

        None
    }
}

/// Errors that can occur when parsing RomFS from bytes
#[derive(Debug, thiserror::Error)]
pub enum FromBytesError {
    /// Buffer is too small to contain the required data
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// Header size field contains invalid value
    #[error("invalid header_size: expected 0x50, found {found:#x}")]
    InvalidHeaderSize {
        /// Found size value
        found: usize,
    },

    /// Directory metadata table offset+size overflows usize
    #[error(
        "dir_meta_table offset+size overflows: offset {offset:#x} + size {size:#x} overflows usize"
    )]
    DirMetaTableBoundsOverflow {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
    },
    /// Directory metadata table extends beyond buffer
    #[error(
        "dir_meta_table out of bounds: offset {offset:#x} + size {size:#x} > buffer length {available:#x}"
    )]
    DirMetaTableOutOfBounds {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
        /// Buffer size
        available: usize,
    },

    /// Directory hash table offset+size overflows usize
    #[error(
        "dir_hash_table offset+size overflows: offset {offset:#x} + size {size:#x} overflows usize"
    )]
    DirHashTableBoundsOverflow {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
    },
    /// Directory hash table extends beyond buffer
    #[error(
        "dir_hash_table out of bounds: offset {offset:#x} + size {size:#x} > buffer length {available:#x}"
    )]
    DirHashTableOutOfBounds {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
        /// Buffer size
        available: usize,
    },

    /// File metadata table offset+size overflows usize
    #[error(
        "file_meta_table offset+size overflows: offset {offset:#x} + size {size:#x} overflows usize"
    )]
    FileMetaTableBoundsOverflow {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
    },
    /// File metadata table extends beyond buffer
    #[error(
        "file_meta_table out of bounds: offset {offset:#x} + size {size:#x} > buffer length {available:#x}"
    )]
    FileMetaTableOutOfBounds {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
        /// Buffer size
        available: usize,
    },

    /// File hash table offset+size overflows usize
    #[error(
        "file_hash_table offset+size overflows: offset {offset:#x} + size {size:#x} overflows usize"
    )]
    FileHashTableBoundsOverflow {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
    },
    /// File hash table extends beyond buffer
    #[error(
        "file_hash_table out of bounds: offset {offset:#x} + size {size:#x} > buffer length {available:#x}"
    )]
    FileHashTableOutOfBounds {
        /// Table offset
        offset: usize,
        /// Table size
        size: usize,
        /// Buffer size
        available: usize,
    },

    /// File data offset is beyond buffer bounds
    #[error("file_data_offset out of bounds: offset {offset:#x} > buffer length {available:#x}")]
    FileDataOffsetOutOfBounds {
        /// Data offset
        offset: usize,
        /// Buffer size
        available: usize,
    },

    /// File data offset+size overflows usize
    ///
    /// This error occurs when calculating the absolute file data position causes arithmetic
    /// overflow. This indicates a malformed RomFS where file metadata contains values that
    /// would overflow when computing file_data_offset + data_offset + data_size.
    #[error(
        "file data offset overflows: file_data_offset {file_data_offset:#x} + data_offset {data_offset:#x} + data_size {data_size:#x} overflows usize"
    )]
    FileDataBoundsOverflow {
        /// Base file data offset from header
        file_data_offset: usize,
        /// File's data offset from entry
        data_offset: usize,
        /// File's data size from entry
        data_size: usize,
    },

    /// File data extends beyond buffer bounds
    ///
    /// This error occurs when a file's data region (offset + size) extends past the end
    /// of the buffer. This indicates a malformed RomFS where file metadata references
    /// data that doesn't exist in the provided buffer.
    #[error(
        "file data out of bounds: file at offset {data_offset:#x} size {data_size:#x} exceeds buffer length {available:#x}"
    )]
    FileDataOutOfBounds {
        /// File's data offset from entry
        data_offset: usize,
        /// File's data size from entry
        data_size: usize,
        /// Buffer size
        available: usize,
    },
}

/// Error when opening root directory
#[derive(Debug, thiserror::Error)]
#[error("buffer too small: need {required} bytes, have {available}")]
pub struct RootDirError {
    /// Number of bytes required
    pub required: usize,
    /// Number of bytes available
    pub available: usize,
}

/// Errors that can occur when opening files/directories in RomFS
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// Buffer is too small when parsing internal structures
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// Path not found
    #[error("path not found")]
    PathNotFound,
    /// Directory not found
    #[error("directory not found")]
    DirNotFound,
    /// File not found
    #[error("file not found")]
    FileNotFound,
}
