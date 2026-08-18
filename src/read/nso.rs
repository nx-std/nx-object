//! Validated reader over an NSO image.
//!
//! [`Nso::try_from_bytes`] checks the magic and proves each segment's compressed
//! extent lies inside the buffer, so the segment accessors return slices without
//! re-checking and cannot panic on a malformed image.
//!
//! The slices they return are the bytes as stored: still compressed when the
//! matching flag is set, and unverified against the header's SHA-256 hashes, which
//! cover the decompressed form. Decompression and hash checking are the caller's.

use zerocopy::FromBytes;

use crate::raw::nso::{NSO_MAGIC, NsoFlags, NsoHeader};

/// A borrowed view of an NSO image whose segment bounds have already been checked.
///
/// The segments it hands back are the bytes as stored, which for a compressed segment is not its
/// contents. Expanding them and checking them against the header's hashes is the caller's, and
/// this type deliberately does neither: it borrows, so it has nowhere to put an expanded segment.
pub struct Nso<'a> {
    bytes: &'a [u8],
    header: &'a NsoHeader,
}

impl<'a> Nso<'a> {
    /// Validate `bytes` as an NSO and borrow it, proving each stored segment lies inside the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is shorter than the header, if the magic
    /// does not match, or if a segment's stored extent runs past the end of the
    /// buffer. The extents checked are the compressed ones actually occupying the
    /// file, so success does not imply a segment decompresses to its declared size.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        if bytes.len() < size_of::<NsoHeader>() {
            return Err(FromBytesError::BufferTooSmall {
                required: size_of::<NsoHeader>(),
                available: bytes.len(),
            });
        }

        let header = NsoHeader::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x100,
                available: bytes.len(),
            })?
            .0;

        if header.magic.get() != NSO_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        // Validate segment bounds to prevent panics in accessor methods
        Self::validate_segment_bounds(
            bytes.len(),
            "text",
            header.text.file_offset.get(),
            header.text_file_size.get(),
        )?;
        Self::validate_segment_bounds(
            bytes.len(),
            "rodata",
            header.rodata.file_offset.get(),
            header.rodata_file_size.get(),
        )?;
        Self::validate_segment_bounds(
            bytes.len(),
            "data",
            header.data.file_offset.get(),
            header.data_file_size.get(),
        )?;

        Ok(Self { bytes, header })
    }

    /// Validate that a segment's offset and size are within buffer bounds
    fn validate_segment_bounds(
        buffer_len: usize,
        segment_name: &'static str,
        offset: u32,
        size: u32,
    ) -> Result<(), FromBytesError> {
        let offset = offset as usize;
        let size = size as usize;

        // Check for overflow when adding offset + size
        let end = offset
            .checked_add(size)
            .ok_or(FromBytesError::SegmentOutOfBounds {
                segment: segment_name,
                offset,
                size,
                buffer_len,
            })?;

        // Check if segment range is within buffer
        if end > buffer_len {
            return Err(FromBytesError::SegmentOutOfBounds {
                segment: segment_name,
                offset,
                size,
                buffer_len,
            });
        }

        Ok(())
    }

    /// Read an NSO already mapped in memory, for a program inspecting its own image.
    ///
    /// The length is not known in advance, so the pointer is treated as the start of an
    /// effectively unbounded slice and the header's own extents decide what is read.
    ///
    /// # Safety
    ///
    /// - `ptr` points at a mapped NSO image, in one allocation with the segments it describes.
    /// - That mapping stays live and unwritten for `'a`.
    ///
    /// # Errors
    ///
    /// Fails on the same conditions as [`Nso::try_from_bytes`]: a magic that is not `NSO0`, or a
    /// segment whose stored extent overflows. A bounds failure against the end of the buffer
    /// cannot be reported here, because the length the pointer stands for is not real.
    pub unsafe fn try_from_ptr(ptr: *const u8) -> Result<Self, FromPtrError> {
        // SAFETY: Caller guarantees ptr is valid and memory remains valid for 'a
        let bytes = unsafe { core::slice::from_raw_parts(ptr, usize::MAX / 2) };
        Self::try_from_bytes(bytes).map_err(FromPtrError)
    }

    /// The header describing the image's segments, their stored lengths, and their hashes.
    pub fn header(&self) -> &NsoHeader {
        self.header
    }

    /// Identity of the build this image was linked from.
    pub fn module_id(&self) -> &[u8; 32] {
        &self.header.module_id
    }

    /// Which segments are stored compressed, and which the loader hash-checks.
    ///
    /// Bits the crate does not know are dropped rather than rejected, so an image using a flag
    /// added after this crate was written still reads.
    pub fn flags(&self) -> NsoFlags {
        NsoFlags::from_bits_truncate(self.header.flags.get())
    }

    /// The `text` segment as stored, still LZ4-compressed when `TEXT_COMPRESS` is set.
    // Slicing is unchecked on purpose: `try_from_bytes` proved the stored extent fits the buffer.
    pub fn text_compressed(&self) -> &'a [u8] {
        let off = self.header.text.file_offset.get() as usize;
        let size = self.header.text_file_size.get() as usize;
        &self.bytes[off..off + size]
    }

    /// The `rodata` segment as stored, still LZ4-compressed when `RODATA_COMPRESS` is set.
    pub fn rodata_compressed(&self) -> &'a [u8] {
        let off = self.header.rodata.file_offset.get() as usize;
        let size = self.header.rodata_file_size.get() as usize;
        &self.bytes[off..off + size]
    }

    /// The `data` segment as stored, still LZ4-compressed when `DATA_COMPRESS` is set.
    pub fn data_compressed(&self) -> &'a [u8] {
        let off = self.header.data.file_offset.get() as usize;
        let size = self.header.data_file_size.get() as usize;
        &self.bytes[off..off + size]
    }

    /// The `text` segment's bytes, expanded when the header marks it compressed.
    ///
    /// The result is the segment as the loader maps it, which includes the zero padding that
    /// rounds it up to a page: the header's size and its hash both cover that padding, so trimming
    /// it here would return bytes the recorded digest does not describe.
    ///
    /// # Errors
    ///
    /// Returns an error if the LZ4 stream is malformed or does not expand to the length the
    /// segment header records.
    #[cfg(feature = "lz4-compression")]
    pub fn text(&self) -> Result<alloc::vec::Vec<u8>, DecompressError> {
        self.decompress(
            Segment::Text,
            self.text_compressed(),
            self.header.text.size.get() as usize,
            NsoFlags::TEXT_COMPRESS,
        )
    }

    /// The `rodata` segment's bytes, expanded when the header marks it compressed.
    ///
    /// # Errors
    ///
    /// Fails on the same conditions as [`Nso::text`].
    #[cfg(feature = "lz4-compression")]
    pub fn rodata(&self) -> Result<alloc::vec::Vec<u8>, DecompressError> {
        self.decompress(
            Segment::RoData,
            self.rodata_compressed(),
            self.header.rodata.size.get() as usize,
            NsoFlags::RODATA_COMPRESS,
        )
    }

    /// The `data` segment's bytes, expanded when the header marks it compressed.
    ///
    /// # Errors
    ///
    /// Fails on the same conditions as [`Nso::text`].
    #[cfg(feature = "lz4-compression")]
    pub fn data(&self) -> Result<alloc::vec::Vec<u8>, DecompressError> {
        self.decompress(
            Segment::Data,
            self.data_compressed(),
            self.header.data.size.get() as usize,
            NsoFlags::DATA_COMPRESS,
        )
    }

    /// Expand `stored` when `flag` is set, or copy it out unchanged when it is not.
    ///
    /// The expanded length comes from the segment header rather than the stream: an NSO stores LZ4
    /// blocks, which do not carry their own decompressed size.
    #[cfg(feature = "lz4-compression")]
    fn decompress(
        &self,
        segment: Segment,
        stored: &[u8],
        decompressed_size: usize,
        flag: NsoFlags,
    ) -> Result<alloc::vec::Vec<u8>, DecompressError> {
        if !self.flags().contains(flag) {
            return Ok(stored.to_vec());
        }

        lz4_flex::decompress(stored, decompressed_size).map_err(|err| DecompressError {
            segment,
            source: err,
        })
    }
}

/// Which segment of an NSO a failure belongs to.
#[cfg(feature = "lz4-compression")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    /// The executable code.
    Text,
    /// The constants and dynamic linking tables.
    RoData,
    /// The writable initialized data.
    Data,
}

#[cfg(feature = "lz4-compression")]
impl core::fmt::Display for Segment {
    /// Renders as the segment's name as the format spells it, so `RoData` prints as `rodata`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::RoData => write!(f, "rodata"),
            Self::Data => write!(f, "data"),
        }
    }
}

/// Error returned by [`Nso::text`], [`Nso::rodata`], and [`Nso::data`].
///
/// Names the segment, because an NSO carries three and the stream alone does not say which failed.
#[cfg(feature = "lz4-compression")]
#[derive(Debug, thiserror::Error)]
#[error("failed to decompress the {segment} segment")]
pub struct DecompressError {
    /// Which segment failed.
    pub segment: Segment,
    /// Why the stream was rejected.
    #[source]
    pub source: lz4_flex::block::DecompressError,
}

/// Errors that can occur when parsing an NSO from bytes
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
    /// Magic number does not match NSO0 (0x304f534e)
    #[error("invalid magic: expected 0x304f534e (NSO0), found {found:#010x}")]
    InvalidMagic {
        /// Found magic number
        found: u32,
    },
    /// Segment file offset and size exceed buffer bounds
    ///
    /// This error occurs when the NSO header specifies a segment (text, rodata, or data)
    /// with a file offset and size that would read beyond the available buffer.
    ///
    /// Common causes:
    /// - Corrupted NSO file with invalid segment descriptors
    /// - Truncated NSO file missing segment data
    /// - Malformed NSO header with deliberately crafted out-of-bounds values
    #[error(
        "{segment} segment out of bounds: offset {offset} + size {size} exceeds buffer length {buffer_len}"
    )]
    SegmentOutOfBounds {
        /// Name of the segment (text, rodata, or data)
        segment: &'static str,
        /// File offset of the segment
        offset: usize,
        /// Size of the segment
        size: usize,
        /// Total buffer length
        buffer_len: usize,
    },
}

/// Error when parsing NSO from raw pointer
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct FromPtrError(FromBytesError);

#[cfg(all(test, feature = "lz4-compression"))]
mod decompress_tests {
    use alloc::{vec, vec::Vec};

    use super::Nso;
    use crate::write::NsoBuilder;

    /// Page size the builder rounds every segment up to before hashing and compressing it.
    const PAGE: usize = 0x1000;

    /// `data` followed by the zero padding that rounds it up to a whole page.
    fn padded(data: &[u8]) -> Vec<u8> {
        let mut padded = data.to_vec();
        padded.resize(data.len().div_ceil(PAGE) * PAGE, 0);
        padded
    }

    /// Build an NSO carrying the three segments, compressed or not as `compressed` says.
    fn nso(text: Vec<u8>, rodata: Vec<u8>, data: Vec<u8>, compressed: bool) -> Vec<u8> {
        NsoBuilder::new()
            .text(text)
            .rodata(rodata)
            .data(data)
            .compressed(compressed)
            .build()
            .expect("a small image should build")
    }

    #[test]
    fn text_with_a_compressed_image_returns_the_segment_that_was_packed() {
        //* Given
        let text = vec![0xAA; 0x1000];
        let image = nso(text.clone(), vec![0xBB; 0x800], vec![0xCC; 0x400], true);
        let parsed = Nso::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segment = parsed.text().expect("a built segment should expand");

        //* Then
        assert_eq!(segment, padded(&text));
    }

    #[test]
    fn rodata_with_a_compressed_image_returns_the_segment_that_was_packed() {
        //* Given
        let rodata = vec![0xBB; 0x800];
        let image = nso(vec![0xAA; 0x1000], rodata.clone(), vec![0xCC; 0x400], true);
        let parsed = Nso::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segment = parsed.rodata().expect("a built segment should expand");

        //* Then
        assert_eq!(segment, padded(&rodata));
    }

    #[test]
    fn data_with_a_compressed_image_returns_the_segment_that_was_packed() {
        //* Given
        let data = vec![0xCC; 0x400];
        let image = nso(vec![0xAA; 0x1000], vec![0xBB; 0x800], data.clone(), true);
        let parsed = Nso::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segment = parsed.data().expect("a built segment should expand");

        //* Then
        assert_eq!(segment, padded(&data));
    }

    #[test]
    fn text_with_an_uncompressed_image_returns_the_stored_bytes() {
        //* Given
        // No compression bit set, so the accessor must copy rather than try to expand.
        let text = vec![0xAA; 0x1000];
        let image = nso(text.clone(), vec![0xBB; 0x800], vec![0xCC; 0x400], false);
        let parsed = Nso::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segment = parsed
            .text()
            .expect("an unpacked segment needs no expanding");

        //* Then
        assert_eq!(segment, padded(&text));
    }

    #[test]
    fn text_with_a_corrupted_stream_fails() {
        //* Given
        // A compressed image whose stored bytes are overwritten: the LZ4 block no longer decodes.
        let image = nso(
            vec![0xAA; 0x1000],
            vec![0xBB; 0x800],
            vec![0xCC; 0x400],
            true,
        );
        let mut corrupted = image.clone();
        let parsed = Nso::try_from_bytes(&image).expect("a built image should parse");
        let offset = parsed.header().text.file_offset.get() as usize;
        corrupted[offset..offset + 0x10].fill(0xFF);
        let parsed = Nso::try_from_bytes(&corrupted).expect("the header is untouched");

        //* When
        let result = parsed.text();

        //* Then
        assert!(result.is_err(), "a corrupted stream must not expand");
    }
}
