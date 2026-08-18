//! Validated reader over a PFS0 archive and the files it holds.
//!
//! [`Pfs0::try_from_bytes`] checks the magic and proves the entry table, the string table, and every
//! file's bounds lie inside the buffer before borrowing it, so the accessors below return slices
//! without re-checking and cannot panic on a malformed archive.
//!
//! Proving every entry up front is affordable here in a way it is not for a RomFS: the entry table
//! is a flat array whose length the header states, so the whole of it is walked in one pass. That is
//! why a file's bytes come back as a plain slice rather than a `Result`.
//!
//! Neither offset in an entry is absolute — the file offset is measured from the start of the data
//! region and the name offset from the start of the string table — so both are resolved here
//! against regions this reader has already located.

use zerocopy::FromBytes as _;

use crate::raw::pfs0::{PFS0_MAGIC, Pfs0FileEntry, Pfs0Header};

/// A borrowed view of a PFS0 archive whose every file has already been bounds-checked.
///
/// Construction is where every check happens, so a `Pfs0` that exists is one whose files lie inside
/// the buffer and whose names are terminated inside the string table.
pub struct Pfs0<'a> {
    header: &'a Pfs0Header,
    entries: &'a [Pfs0FileEntry],
    string_table: &'a [u8],
    data: &'a [u8],
}

impl<'a> Pfs0<'a> {
    /// Validate `bytes` as a PFS0 and borrow it, proving every file lies inside the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot hold the header, the entry table, or the string table,
    /// if the magic does not match, if a file's offset and size overflow or run past the end of the
    /// data region, or if a name starts outside the string table or is never terminated inside it.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        let (header, rest) =
            Pfs0Header::ref_from_prefix(bytes).map_err(|_| FromBytesError::BufferTooSmall {
                required: size_of::<Pfs0Header>(),
                available: bytes.len(),
            })?;

        if header.magic.get() != PFS0_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        let file_count = header.file_count.get() as usize;
        let string_table_size = header.string_table_size.get() as usize;

        let (entries, rest) = <[Pfs0FileEntry]>::ref_from_prefix_with_elems(rest, file_count)
            .map_err(|_| FromBytesError::EntryTableOutOfBounds {
                file_count,
                available: bytes.len(),
            })?;

        let string_table =
            rest.get(..string_table_size)
                .ok_or(FromBytesError::StringTableOutOfBounds {
                    string_table_size,
                    available: rest.len(),
                })?;

        // Indexing past the string table is what the check above just proved.
        let data = &rest[string_table_size..];

        for (index, entry) in entries.iter().enumerate() {
            check_file_bounds(index, entry, data.len())?;
            check_name_bounds(index, entry, string_table)?;
        }

        Ok(Self {
            header,
            entries,
            string_table,
            data,
        })
    }

    /// The header stating how many files the archive holds and how long its string table is.
    pub fn header(&self) -> &Pfs0Header {
        self.header
    }

    /// Number of files in the archive.
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// The file at `index`, or `None` when the archive holds fewer files than that.
    pub fn file(&self, index: usize) -> Option<Pfs0File<'a>> {
        let entry = self.entries.get(index)?;
        Some(self.resolve(entry))
    }

    /// Every file in the archive, in the order the entry table lists them.
    pub fn files(&self) -> impl Iterator<Item = Pfs0File<'a>> + '_ {
        self.entries.iter().map(|entry| self.resolve(entry))
    }

    /// The file stored under `name`, or `None` when the archive holds no such entry.
    ///
    /// A name that is not valid UTF-8 reads as empty, so a malformed entry cannot match a lookup.
    pub fn file_by_name(&self, name: &str) -> Option<Pfs0File<'a>> {
        self.files().find(|file| file.name() == name)
    }

    // Every bound reached here was proven by `try_from_bytes`, which is what lets this return a
    // file rather than a `Result`.
    fn resolve(&self, entry: &'a Pfs0FileEntry) -> Pfs0File<'a> {
        let offset = entry.offset.get() as usize;
        let size = entry.size.get() as usize;
        let name_offset = entry.string_table_offset.get() as usize;

        let name_bytes = &self.string_table[name_offset..];
        let name_end = name_bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name_bytes.len());

        Pfs0File {
            name: core::str::from_utf8(&name_bytes[..name_end]).unwrap_or(""),
            data: &self.data[offset..offset + size],
        }
    }
}

/// Prove one entry's bytes lie inside the data region.
fn check_file_bounds(
    index: usize,
    entry: &Pfs0FileEntry,
    available: usize,
) -> Result<(), FromBytesError> {
    let offset = entry.offset.get() as usize;
    let size = entry.size.get() as usize;

    let end = offset
        .checked_add(size)
        .ok_or(FromBytesError::FileBoundsOverflow {
            file_index: index,
            offset,
            size,
        })?;

    if end > available {
        return Err(FromBytesError::FileOutOfBounds {
            file_index: index,
            offset,
            size,
            available,
        });
    }

    Ok(())
}

/// Prove one entry's name starts inside the string table and is terminated within it.
fn check_name_bounds(
    index: usize,
    entry: &Pfs0FileEntry,
    string_table: &[u8],
) -> Result<(), FromBytesError> {
    let offset = entry.string_table_offset.get() as usize;

    let name_bytes = string_table
        .get(offset..)
        .ok_or(FromBytesError::NameOutOfBounds {
            file_index: index,
            offset,
            string_table_size: string_table.len(),
        })?;

    if !name_bytes.contains(&0) {
        return Err(FromBytesError::NameUnterminated {
            file_index: index,
            offset,
        });
    }

    Ok(())
}

/// Errors that can occur when parsing a PFS0 from bytes
#[derive(Debug, thiserror::Error)]
pub enum FromBytesError {
    /// Buffer is too small to contain the header
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// Magic number does not match PFS0 (0x30534650)
    #[error("invalid PFS0 magic: found {found:#010x}")]
    InvalidMagic {
        /// The magic that was found in place of `PFS0`
        found: u32,
    },
    /// The entry table the header declares does not fit in the buffer
    #[error("entry table of {file_count} files does not fit in {available} bytes")]
    EntryTableOutOfBounds {
        /// Number of files the header declares
        file_count: usize,
        /// Number of bytes the whole archive occupies
        available: usize,
    },
    /// The string table the header declares does not fit in the buffer
    #[error("string table of {string_table_size} bytes does not fit in {available} bytes")]
    StringTableOutOfBounds {
        /// Length the header declares for the string table
        string_table_size: usize,
        /// Number of bytes left after the entry table
        available: usize,
    },
    /// A file's offset and size overflow when added
    #[error("file {file_index} has offset {offset} and size {size}, which overflow")]
    FileBoundsOverflow {
        /// Index of the file in the entry table
        file_index: usize,
        /// Offset the entry records, from the start of the data region
        offset: usize,
        /// Size the entry records
        size: usize,
    },
    /// A file extends past the end of the data region
    #[error("file {file_index} at {offset} of size {size} runs past the {available} available")]
    FileOutOfBounds {
        /// Index of the file in the entry table
        file_index: usize,
        /// Offset the entry records, from the start of the data region
        offset: usize,
        /// Size the entry records
        size: usize,
        /// Number of bytes the data region holds
        available: usize,
    },
    /// A file's name starts past the end of the string table
    #[error(
        "file {file_index} names an offset {offset} outside the {string_table_size}-byte table"
    )]
    NameOutOfBounds {
        /// Index of the file in the entry table
        file_index: usize,
        /// Offset the entry records, from the start of the string table
        offset: usize,
        /// Length of the string table
        string_table_size: usize,
    },
    /// A file's name has no terminator before the end of the string table
    #[error("file {file_index} has an unterminated name at offset {offset}")]
    NameUnterminated {
        /// Index of the file in the entry table
        file_index: usize,
        /// Offset the entry records, from the start of the string table
        offset: usize,
    },
}

/// One file in the archive, with its name and its bytes already resolved.
pub struct Pfs0File<'a> {
    name: &'a str,
    data: &'a [u8],
}

impl<'a> Pfs0File<'a> {
    /// The name the file is stored under.
    ///
    /// A name that is not valid UTF-8 reads as empty rather than failing, so a malformed entry
    /// cannot match a lookup.
    pub fn name(&self) -> &'a str {
        self.name
    }

    /// The file's bytes, borrowed from the archive.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use alloc::vec::Vec;

    use super::{FromBytesError, Pfs0};

    /// Build a PFS0 holding `files`, laid out the way the format requires.
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut string_table = Vec::new();
        let mut name_offsets = Vec::new();
        for (name, _) in files {
            name_offsets.push(string_table.len() as u32);
            string_table.extend_from_slice(name.as_bytes());
            string_table.push(0);
        }
        while string_table.len() % 0x20 != 0 {
            string_table.push(0);
        }

        let mut entries = Vec::new();
        let mut offset = 0u64;
        for ((_, data), name_offset) in files.iter().zip(name_offsets) {
            entries.extend_from_slice(&offset.to_le_bytes());
            entries.extend_from_slice(&(data.len() as u64).to_le_bytes());
            entries.extend_from_slice(&name_offset.to_le_bytes());
            entries.extend_from_slice(&0u32.to_le_bytes());
            offset += data.len() as u64;
        }

        let mut image = Vec::new();
        image.extend_from_slice(&0x3053_4650u32.to_le_bytes());
        image.extend_from_slice(&(files.len() as u32).to_le_bytes());
        image.extend_from_slice(&(string_table.len() as u32).to_le_bytes());
        image.extend_from_slice(&0u32.to_le_bytes());
        image.extend_from_slice(&entries);
        image.extend_from_slice(&string_table);
        for (_, data) in files {
            image.extend_from_slice(data);
        }
        image
    }

    #[test]
    fn try_from_bytes_with_two_files_resolves_both() {
        //* Given
        let image = archive(&[("main", b"code"), ("main.npdm", b"descriptor")]);

        //* When
        let pfs0 = Pfs0::try_from_bytes(&image).expect("a well-formed archive should parse");

        //* Then
        assert_eq!(pfs0.file_count(), 2);
        let main = pfs0.file(0).expect("the first entry should resolve");
        assert_eq!(main.name(), "main");
        assert_eq!(main.data(), b"code");
        let npdm = pfs0.file(1).expect("the second entry should resolve");
        assert_eq!(npdm.name(), "main.npdm");
        assert_eq!(npdm.data(), b"descriptor");
    }

    #[test]
    fn try_from_bytes_with_no_files_succeeds() {
        //* Given
        let image = archive(&[]);

        //* When
        let pfs0 = Pfs0::try_from_bytes(&image).expect("an empty archive should parse");

        //* Then
        assert_eq!(pfs0.file_count(), 0);
    }

    #[test]
    fn try_from_bytes_with_wrong_magic_fails() {
        //* Given
        let mut image = archive(&[("main", b"code")]);
        image[0] = b'X';

        //* When
        let result = Pfs0::try_from_bytes(&image);

        //* Then
        assert!(matches!(result, Err(FromBytesError::InvalidMagic { .. })));
    }

    #[test]
    fn try_from_bytes_with_a_file_past_the_data_region_fails() {
        //* Given
        // The entry's size is widened past what the archive actually carries.
        let mut image = archive(&[("main", b"code")]);
        let size_at = 0x10 + 0x8;
        image[size_at..size_at + 8].copy_from_slice(&0x1000u64.to_le_bytes());

        //* When
        let result = Pfs0::try_from_bytes(&image);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::FileOutOfBounds { .. })
        ));
    }

    #[test]
    fn try_from_bytes_with_a_name_outside_the_string_table_fails() {
        //* Given
        let mut image = archive(&[("main", b"code")]);
        let name_offset_at = 0x10 + 0x10;
        image[name_offset_at..name_offset_at + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

        //* When
        let result = Pfs0::try_from_bytes(&image);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::NameOutOfBounds { .. })
        ));
    }

    #[test]
    fn try_from_bytes_with_a_truncated_entry_table_fails() {
        //* Given
        // The header claims a file the buffer has no entry for.
        let mut image = archive(&[]);
        image[0x4..0x8].copy_from_slice(&1u32.to_le_bytes());

        //* When
        let result = Pfs0::try_from_bytes(&image);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::EntryTableOutOfBounds { .. })
        ));
    }

    #[test]
    fn file_by_name_with_a_name_the_archive_holds_returns_it() {
        //* Given
        let image = archive(&[("main", b"code"), ("main.npdm", b"descriptor")]);
        let pfs0 = Pfs0::try_from_bytes(&image).expect("a well-formed archive should parse");

        //* When
        let file = pfs0.file_by_name("main.npdm");

        //* Then
        assert_eq!(
            file.expect("the archive holds this name").data(),
            b"descriptor"
        );
    }

    #[test]
    fn file_by_name_with_a_name_the_archive_lacks_returns_none() {
        //* Given
        let image = archive(&[("main", b"code")]);
        let pfs0 = Pfs0::try_from_bytes(&image).expect("a well-formed archive should parse");

        //* When
        let file = pfs0.file_by_name("logo");

        //* Then
        assert!(file.is_none());
    }
}
