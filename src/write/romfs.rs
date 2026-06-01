//! RomFS (Read-only filesystem) builder.

use std::{path::PathBuf, string::String, vec::Vec};

use crate::raw::romfs::{RomFsDirEntry, RomFsFileEntry};

const ROMFS_FILEPARTITION_OFS: u64 = 0x200;
const ROMFS_ENTRY_EMPTY: u32 = 0xFFFFFFFF;

/// Internal directory entry for building RomFS.
struct DirEntry {
    name: String,
    entry_offset: u32,
    children: Vec<usize>,
    files: Vec<usize>,
    parent: usize,
}

/// Internal file entry for building RomFS.
struct FileEntry {
    name: String,
    data: Vec<u8>,
    entry_offset: u32,
    data_offset: u64,
    parent: usize,
}

/// Builder for constructing RomFS images.
pub struct RomFsBuilder {
    dirs: Vec<DirEntry>,
    files: Vec<FileEntry>,
    dir_hash_siblings: Vec<u32>,
    file_hash_siblings: Vec<u32>,
}

impl RomFsBuilder {
    /// Create a new RomFS builder.
    pub fn new() -> Self {
        // Initialize with root directory
        Self {
            dirs: vec![DirEntry {
                name: String::new(),
                entry_offset: 0,
                children: Vec::new(),
                files: Vec::new(),
                parent: 0, // root parent is itself
            }],
            files: Vec::new(),
            dir_hash_siblings: Vec::new(),
            file_hash_siblings: Vec::new(),
        }
    }

    /// Add a file to the RomFS image.
    ///
    /// The path should start with `/` and use `/` as the separator.
    pub fn add_file(
        mut self,
        path: impl Into<String>,
        data: impl Into<Vec<u8>>,
    ) -> Result<Self, BuildError> {
        let path = path.into();
        let data = data.into();

        // Validate path
        if !path.starts_with('/') {
            return Err(BuildError::InvalidPath { path: path.clone() });
        }

        // Parse path components
        let components: Vec<&str> = path[1..].split('/').filter(|s| !s.is_empty()).collect();
        if components.is_empty() {
            return Err(BuildError::InvalidPath { path: path.clone() });
        }

        let file_name = components[components.len() - 1];
        let dir_components = &components[..components.len() - 1];

        // Navigate/create directory tree
        let mut current_dir = 0;
        for &component in dir_components {
            let dir_name = component.to_string();

            // Find or create directory
            let child_idx = self.dirs[current_dir]
                .children
                .iter()
                .find(|&&idx| self.dirs[idx].name == dir_name)
                .copied();

            current_dir = if let Some(idx) = child_idx {
                idx
            } else {
                // Create new directory
                let new_idx = self.dirs.len();
                self.dirs.push(DirEntry {
                    name: dir_name,
                    entry_offset: 0,
                    children: Vec::new(),
                    files: Vec::new(),
                    parent: current_dir,
                });
                self.dirs[current_dir].children.push(new_idx);
                new_idx
            };
        }

        // Add file
        let file_name = file_name.to_string();

        // Check for duplicate
        for &file_idx in &self.dirs[current_dir].files {
            if self.files[file_idx].name == file_name {
                return Err(BuildError::DuplicateEntry { path: path.clone() });
            }
        }

        let file_idx = self.files.len();
        self.files.push(FileEntry {
            name: file_name,
            data,
            entry_offset: 0,
            data_offset: 0,
            parent: current_dir,
        });
        self.dirs[current_dir].files.push(file_idx);

        Ok(self)
    }

    /// Build from a directory on the filesystem.
    ///
    /// Recursively adds all files from the given directory.
    #[cfg(feature = "std")]
    pub fn from_directory(path: impl AsRef<std::path::Path>) -> Result<Self, FromDirectoryError> {
        use fs_err as fs;

        let mut builder = Self::new();
        let base_path = path.as_ref();

        // Stack of (dir_idx, filesystem_path)
        let mut stack = vec![(0, base_path.to_path_buf())];

        while let Some((dir_idx, fs_path)) = stack.pop() {
            let entries = fs::read_dir(&fs_path).map_err(|source| FromDirectoryError::Io {
                path: fs_path.clone(),
                source,
            })?;

            for entry in entries {
                let entry = entry.map_err(|source| FromDirectoryError::Io {
                    path: fs_path.clone(),
                    source,
                })?;

                let file_type = entry.file_type().map_err(|source| FromDirectoryError::Io {
                    path: entry.path(),
                    source,
                })?;

                let name = entry
                    .file_name()
                    .to_str()
                    .ok_or_else(|| FromDirectoryError::InvalidFileName { path: entry.path() })?
                    .to_string();

                if file_type.is_dir() {
                    let new_idx = builder.dirs.len();
                    builder.dirs.push(DirEntry {
                        name,
                        entry_offset: 0,
                        children: Vec::new(),
                        files: Vec::new(),
                        parent: dir_idx,
                    });
                    builder.dirs[dir_idx].children.push(new_idx);
                    stack.push((new_idx, entry.path()));
                } else if file_type.is_file() {
                    let data = fs::read(entry.path()).map_err(|source| FromDirectoryError::Io {
                        path: entry.path(),
                        source,
                    })?;

                    let file_idx = builder.files.len();
                    builder.files.push(FileEntry {
                        name,
                        data,
                        entry_offset: 0,
                        data_offset: 0,
                        parent: dir_idx,
                    });
                    builder.dirs[dir_idx].files.push(file_idx);
                } else if file_type.is_symlink() {
                    return Err(FromDirectoryError::Symlink { path: entry.path() });
                }
            }
        }

        Ok(builder)
    }

    /// Build the complete RomFS image.
    pub fn build(mut self) -> Result<Vec<u8>, BuildError> {
        if self.files.is_empty() {
            return Err(BuildError::Empty);
        }

        // Sort children and files by name
        for i in 0..self.dirs.len() {
            // Collect names for sorting
            let mut child_names: Vec<(usize, String)> = self.dirs[i]
                .children
                .iter()
                .map(|&idx| (idx, self.dirs[idx].name.clone()))
                .collect();
            child_names.sort_by(|(_, a), (_, b)| a.cmp(b));
            self.dirs[i].children = child_names.into_iter().map(|(idx, _)| idx).collect();

            let mut file_names: Vec<(usize, String)> = self.dirs[i]
                .files
                .iter()
                .map(|&idx| (idx, self.files[idx].name.clone()))
                .collect();
            file_names.sort_by(|(_, a), (_, b)| a.cmp(b));
            self.dirs[i].files = file_names.into_iter().map(|(idx, _)| idx).collect();
        }

        // Calculate offsets
        self.calculate_offsets();

        // Initialize hash sibling tracking
        self.dir_hash_siblings = vec![ROMFS_ENTRY_EMPTY; self.dirs.len()];
        self.file_hash_siblings = vec![ROMFS_ENTRY_EMPTY; self.files.len()];

        // Generate hash tables
        let dir_hash_table = self.generate_dir_hash_table();
        let file_hash_table = self.generate_file_hash_table();

        // Build metadata tables
        let dir_table = self.build_dir_table();
        let file_table = self.build_file_table();

        // Calculate sizes
        let file_partition_size = self.calculate_file_partition_size();
        let total_size = (ROMFS_FILEPARTITION_OFS
            + align64(file_partition_size, 4)
            + (dir_hash_table.len() * 4) as u64
            + dir_table.len() as u64
            + (file_hash_table.len() * 4) as u64
            + file_table.len() as u64) as usize;

        let mut buf = Vec::with_capacity(total_size);

        // Write header
        self.write_header(
            &mut buf,
            &dir_hash_table,
            &dir_table,
            &file_hash_table,
            &file_table,
            file_partition_size,
        );

        // Pad to 0x200
        buf.resize(ROMFS_FILEPARTITION_OFS as usize, 0);

        // Write file data
        for file in &self.files {
            // Align to 0x10
            let aligned_offset = align64(buf.len() as u64, 0x10) as usize;
            buf.resize(aligned_offset, 0);

            buf.extend_from_slice(&file.data);
        }

        // Pad to 4-byte alignment
        let aligned_offset = align64(buf.len() as u64, 4) as usize;
        buf.resize(aligned_offset, 0);

        // Write hash tables and metadata tables
        for &hash in &dir_hash_table {
            buf.extend_from_slice(&hash.to_le_bytes());
        }
        buf.extend_from_slice(&dir_table);

        for &hash in &file_hash_table {
            buf.extend_from_slice(&hash.to_le_bytes());
        }
        buf.extend_from_slice(&file_table);

        Ok(buf)
    }

    fn calculate_offsets(&mut self) {
        // Calculate file data offsets
        let mut data_offset = 0u64;
        for file in &mut self.files {
            data_offset = align64(data_offset, 0x10);
            file.data_offset = data_offset;
            data_offset += file.data.len() as u64;
        }

        // Calculate file entry offsets
        let mut entry_offset = 0u32;
        for file in &mut self.files {
            file.entry_offset = entry_offset;
            entry_offset += (core::mem::size_of::<RomFsFileEntry>() as u32)
                + align32(file.name.len() as u32, 4);
        }

        // Calculate directory entry offsets
        let mut entry_offset = 0u32;
        for dir in &mut self.dirs {
            dir.entry_offset = entry_offset;
            entry_offset +=
                (core::mem::size_of::<RomFsDirEntry>() as u32) + align32(dir.name.len() as u32, 4);
        }
    }

    fn calculate_file_partition_size(&self) -> u64 {
        let mut size = 0u64;
        for file in &self.files {
            size = align64(size, 0x10);
            size += file.data.len() as u64;
        }
        size
    }

    fn generate_dir_hash_table(&mut self) -> Vec<u32> {
        let table_size = romfs_get_hash_table_count(self.dirs.len());
        let mut table = vec![ROMFS_ENTRY_EMPTY; table_size];

        for (dir_idx, dir) in self.dirs.iter().enumerate() {
            let parent_offset = self.dirs[dir.parent].entry_offset;
            let hash = calc_path_hash(parent_offset, &dir.name);
            let bucket = (hash as usize) % table_size;

            // Chain collision: new entry becomes head, points to old head
            let old_head = table[bucket];
            table[bucket] = dir.entry_offset;
            self.dir_hash_siblings[dir_idx] = old_head;
        }

        table
    }

    fn generate_file_hash_table(&mut self) -> Vec<u32> {
        let table_size = romfs_get_hash_table_count(self.files.len());
        let mut table = vec![ROMFS_ENTRY_EMPTY; table_size];

        for (file_idx, file) in self.files.iter().enumerate() {
            let parent_offset = self.dirs[file.parent].entry_offset;
            let hash = calc_path_hash(parent_offset, &file.name);
            let bucket = (hash as usize) % table_size;

            // Chain collision: new entry becomes head, points to old head
            let old_head = table[bucket];
            table[bucket] = file.entry_offset;
            self.file_hash_siblings[file_idx] = old_head;
        }

        table
    }

    fn build_dir_table(&self) -> Vec<u8> {
        let mut table = Vec::new();

        for (dir_idx, dir) in self.dirs.iter().enumerate() {
            let parent_offset = self.dirs[dir.parent].entry_offset;
            let sibling_offset = self.find_sibling_dir(dir);
            let first_child = dir
                .children
                .first()
                .map(|&idx| self.dirs[idx].entry_offset)
                .unwrap_or(ROMFS_ENTRY_EMPTY);
            let first_file = dir
                .files
                .first()
                .map(|&idx| self.files[idx].entry_offset)
                .unwrap_or(ROMFS_ENTRY_EMPTY);

            let hash_sibling = self.dir_hash_siblings[dir_idx];

            // Write entry header
            table.extend_from_slice(&parent_offset.to_le_bytes());
            table.extend_from_slice(&sibling_offset.to_le_bytes());
            table.extend_from_slice(&first_child.to_le_bytes());
            table.extend_from_slice(&first_file.to_le_bytes());
            table.extend_from_slice(&hash_sibling.to_le_bytes());
            table.extend_from_slice(&(dir.name.len() as u32).to_le_bytes());

            // Write name
            table.extend_from_slice(dir.name.as_bytes());

            // Pad to 4-byte alignment
            let padded_len = align32(table.len() as u32, 4) as usize;
            table.resize(padded_len, 0);
        }

        table
    }

    fn build_file_table(&self) -> Vec<u8> {
        let mut table = Vec::new();

        for (file_idx, file) in self.files.iter().enumerate() {
            let parent_offset = self.dirs[file.parent].entry_offset;
            let sibling_offset = self.find_sibling_file(file);

            let hash_sibling = self.file_hash_siblings[file_idx];

            // Write entry header
            table.extend_from_slice(&parent_offset.to_le_bytes());
            table.extend_from_slice(&sibling_offset.to_le_bytes());
            table.extend_from_slice(&file.data_offset.to_le_bytes());
            table.extend_from_slice(&(file.data.len() as u64).to_le_bytes());
            table.extend_from_slice(&hash_sibling.to_le_bytes());
            table.extend_from_slice(&(file.name.len() as u32).to_le_bytes());

            // Write name
            table.extend_from_slice(file.name.as_bytes());

            // Pad to 4-byte alignment
            let padded_len = align32(table.len() as u32, 4) as usize;
            table.resize(padded_len, 0);
        }

        table
    }

    fn find_sibling_dir(&self, dir: &DirEntry) -> u32 {
        let parent = &self.dirs[dir.parent];
        parent
            .children
            .windows(2)
            .find(|window| self.dirs[window[0]].entry_offset == dir.entry_offset)
            .map(|window| self.dirs[window[1]].entry_offset)
            .unwrap_or(ROMFS_ENTRY_EMPTY)
    }

    fn find_sibling_file(&self, file: &FileEntry) -> u32 {
        let parent = &self.dirs[file.parent];
        parent
            .files
            .windows(2)
            .find(|window| self.files[window[0]].entry_offset == file.entry_offset)
            .map(|window| self.files[window[1]].entry_offset)
            .unwrap_or(ROMFS_ENTRY_EMPTY)
    }

    fn write_header(
        &self,
        buf: &mut Vec<u8>,
        dir_hash_table: &[u32],
        dir_table: &[u8],
        file_hash_table: &[u32],
        file_table: &[u8],
        file_partition_size: u64,
    ) {
        let header_size = 0x50u64;

        let file_data_end = ROMFS_FILEPARTITION_OFS + file_partition_size;
        let dir_hash_offset = align64(file_data_end, 4);
        let dir_hash_size = (dir_hash_table.len() * 4) as u64;

        let dir_table_offset = dir_hash_offset + dir_hash_size;
        let dir_table_size = dir_table.len() as u64;

        let file_hash_offset = dir_table_offset + dir_table_size;
        let file_hash_size = (file_hash_table.len() * 4) as u64;

        let file_table_offset = file_hash_offset + file_hash_size;
        let file_table_size = file_table.len() as u64;

        // Write header
        buf.extend_from_slice(&header_size.to_le_bytes());
        buf.extend_from_slice(&dir_hash_offset.to_le_bytes());
        buf.extend_from_slice(&dir_hash_size.to_le_bytes());
        buf.extend_from_slice(&dir_table_offset.to_le_bytes());
        buf.extend_from_slice(&dir_table_size.to_le_bytes());
        buf.extend_from_slice(&file_hash_offset.to_le_bytes());
        buf.extend_from_slice(&file_hash_size.to_le_bytes());
        buf.extend_from_slice(&file_table_offset.to_le_bytes());
        buf.extend_from_slice(&file_table_size.to_le_bytes());
        buf.extend_from_slice(&ROMFS_FILEPARTITION_OFS.to_le_bytes());
    }
}

impl Default for RomFsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned by [`RomFsBuilder::build`].
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// `build` was called without any files added.
    ///
    /// A RomFS image must contain at least one file; an empty builder has
    /// nothing to serialize.
    #[error("empty romfs (no files added)")]
    Empty,
    /// A registered path is not a valid RomFS path.
    ///
    /// Raised when a path cannot be represented in the RomFS tree, for example
    /// because it is not absolute or contains an empty component. Holds the
    /// offending path.
    #[error("invalid path: {path}")]
    InvalidPath { path: String },
    /// Two entries resolve to the same path.
    ///
    /// Raised when a file or directory is added more than once. Holds the
    /// duplicated path.
    #[error("duplicate entry: {path}")]
    DuplicateEntry { path: String },
}

/// Error returned by [`RomFsBuilder::from_directory`].
#[cfg(feature = "std")]
#[derive(Debug, thiserror::Error)]
pub enum FromDirectoryError {
    /// A filesystem entry could not be read while walking the directory tree.
    ///
    /// Raised when reading directory entries or file contents fails. Holds the
    /// path being read and the underlying [`std::io::Error`].
    #[error("I/O error reading {}", path.display())]
    Io {
        /// Path that was being read when the I/O error occurred.
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The directory tree contains a symbolic link.
    ///
    /// RomFS has no representation for symlinks, so they cannot be packed. Holds
    /// the path of the symlink.
    #[error("symlinks not supported: {}", path.display())]
    Symlink { path: PathBuf },
    /// A file or directory name is not valid UTF-8.
    ///
    /// RomFS stores names as UTF-8; an entry whose name cannot be decoded is
    /// rejected. Holds the offending path.
    #[error("invalid file name: {}", path.display())]
    InvalidFileName { path: PathBuf },
}

/// Calculate hash table size using pseudo-prime algorithm.
fn romfs_get_hash_table_count(num_entries: usize) -> usize {
    if num_entries < 3 {
        3
    } else if num_entries < 19 {
        num_entries | 1
    } else {
        let mut n = num_entries;
        while n.is_multiple_of(2)
            || n.is_multiple_of(3)
            || n.is_multiple_of(5)
            || n.is_multiple_of(7)
            || n.is_multiple_of(11)
            || n.is_multiple_of(13)
            || n.is_multiple_of(17)
        {
            n += 1;
        }
        n
    }
}

/// Calculate path hash for hash table lookup.
fn calc_path_hash(parent_offset: u32, name: &str) -> u32 {
    let mut hash = parent_offset ^ 123_456_789;
    for c in name.bytes() {
        hash = hash.rotate_right(5) ^ u32::from(c);
    }
    hash
}

/// Align value to specified alignment.
fn align32(value: u32, alignment: u32) -> u32 {
    let mask = alignment - 1;
    (value + mask) & !mask
}

/// Align value to specified alignment.
fn align64(value: u64, alignment: u64) -> u64 {
    let mask = alignment - 1;
    (value + mask) & !mask
}
