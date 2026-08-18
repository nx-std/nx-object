//! Validated reader over a KIP1 image and the segments it carries.
//!
//! [`Kip1::try_from_bytes`] checks the magic and proves every segment's stored bytes lie inside the
//! buffer before borrowing it, so the segment accessors return slices without re-checking.
//!
//! Segments come back **as stored**, which for a KIP1 usually means BLZ-compressed: the header's
//! flags say which of the first three are, and [`Kip1Segment::decompress`] is the one accessor that
//! allocates. That split is the same one the rest of the crate draws — a borrowing reader has
//! nowhere to put expanded bytes, so expanding is an explicit, fallible step.
//!
//! `bss` is the exception among the four: it occupies no bytes in the file at all, so it has a
//! destination and a length but nothing to read. It is reported with an empty slice rather than
//! omitted, because the loader still has to reserve room for it.

use alloc::vec::Vec;

use zerocopy::FromBytes as _;

use crate::{
    blz,
    raw::kip::{KIP1_MAGIC, Kip1Header, Kip1Segment as RawKip1Segment},
};

/// Number of segments a KIP1 actually loads: `text`, `rodata`, `data`, and `bss`.
const LOADED_SEGMENT_COUNT: usize = 4;

/// Index of the `bss` segment, which is the one with no bytes in the file.
const BSS_INDEX: usize = 3;

/// A borrowed view of a KIP1 image whose every segment has already been bounds-checked.
///
/// Construction is where every check happens, so a `Kip1` that exists is one whose stored segments
/// lie inside the buffer.
pub struct Kip1<'a> {
    header: &'a Kip1Header,
    bytes: &'a [u8],
}

impl<'a> Kip1<'a> {
    /// Validate `bytes` as a KIP1 and borrow it, proving every stored segment lies inside it.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot hold the `0x100`-byte header, if the magic does not
    /// match, or if a segment's stored bytes overflow or run past the end of the buffer.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        let header = Kip1Header::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: size_of::<Kip1Header>(),
                available: bytes.len(),
            })?
            .0;

        if header.magic.get() != KIP1_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        // Segments are stored back to back after the header, in descriptor order, so each one's
        // offset is the sum of the stored lengths before it.
        let mut offset = size_of::<Kip1Header>();
        for index in 0..LOADED_SEGMENT_COUNT {
            let stored = stored_size(header, index);
            let end = offset
                .checked_add(stored)
                .ok_or(FromBytesError::SegmentBoundsOverflow {
                    segment_index: index,
                    offset,
                    size: stored,
                })?;

            if end > bytes.len() {
                return Err(FromBytesError::SegmentOutOfBounds {
                    segment_index: index,
                    offset,
                    size: stored,
                    available: bytes.len(),
                });
            }

            offset = end;
        }

        Ok(Self { header, bytes })
    }

    /// The header describing the process, its segments, and its capabilities.
    pub fn header(&self) -> &Kip1Header {
        self.header
    }

    /// The process name, with the padding the header stores it under trimmed.
    ///
    /// A name that is not valid UTF-8 reads as empty rather than failing, which matches how the
    /// other readers in this crate treat a malformed name.
    pub fn name(&self) -> &'a str {
        let name = &self.header.name;
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        core::str::from_utf8(&name[..end]).unwrap_or("")
    }

    /// The title the process runs under.
    pub fn title_id(&self) -> u64 {
        self.header.title_id.get()
    }

    /// The segment at `index`, or `None` for one of the two descriptors a KIP1 leaves unused.
    pub fn segment(&self, index: usize) -> Option<Kip1Segment<'a>> {
        if index >= LOADED_SEGMENT_COUNT {
            return None;
        }

        // Every bound below was proven by `try_from_bytes`.
        let mut offset = size_of::<Kip1Header>();
        for before in 0..index {
            offset += stored_size(self.header, before);
        }
        let stored = stored_size(self.header, index);

        Some(Kip1Segment {
            index,
            descriptor: &self.header.segments[index],
            compressed: is_compressed(self.header, index),
            stored: &self.bytes[offset..offset + stored],
        })
    }

    /// The four segments a KIP1 loads, in descriptor order.
    pub fn segments(&self) -> impl Iterator<Item = Kip1Segment<'a>> + '_ {
        (0..LOADED_SEGMENT_COUNT).filter_map(|index| self.segment(index))
    }
}

/// One segment: where it lands, how it is stored, and whether it is packed.
pub struct Kip1Segment<'a> {
    index: usize,
    descriptor: &'a RawKip1Segment,
    compressed: bool,
    stored: &'a [u8],
}

impl<'a> Kip1Segment<'a> {
    /// Which of the four loaded segments this is: `0` text, `1` rodata, `2` data, `3` bss.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The descriptor recording where the segment lands and how large it is.
    pub fn descriptor(&self) -> &'a RawKip1Segment {
        self.descriptor
    }

    /// Address the segment is loaded at, relative to the process image base.
    pub fn address(&self) -> u32 {
        self.descriptor.dst_addr.get()
    }

    /// Length of the segment once expanded, which for `bss` is the room to reserve.
    pub fn decompressed_size(&self) -> usize {
        self.descriptor.decomp_size.get() as usize
    }

    /// Whether the stored bytes are BLZ-packed.
    ///
    /// Only the first three segments can be: `bss` has no bytes to pack.
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// The segment exactly as the file stores it, packed or not.
    ///
    /// Empty for `bss`, which occupies no bytes in the file.
    pub fn stored(&self) -> &'a [u8] {
        self.stored
    }

    /// The segment's bytes, expanded if the header marks it packed.
    ///
    /// A segment that is not packed is copied out unchanged, so a caller that wants the loadable
    /// bytes does not have to branch on [`Kip1Segment::is_compressed`] first.
    ///
    /// # Errors
    ///
    /// Returns an error if the BLZ stream is malformed, or if it does not expand to the length the
    /// descriptor records — which means the header and the segment disagree.
    pub fn decompress(&self) -> Result<Vec<u8>, DecompressError> {
        if !self.compressed {
            return Ok(self.stored.to_vec());
        }

        blz::decompress(self.stored, self.decompressed_size()).map_err(|err| DecompressError {
            segment_index: self.index,
            source: err,
        })
    }
}

/// Error returned by [`Kip1Segment::decompress`].
///
/// Names the segment, because a KIP1 carries three packed ones and the stream alone does not say
/// which failed.
#[derive(Debug, thiserror::Error)]
#[error("failed to decompress segment {segment_index}")]
pub struct DecompressError {
    /// Which of the four loaded segments failed.
    pub segment_index: usize,
    /// Why the stream was rejected.
    #[source]
    pub source: blz::DecompressError,
}

/// Bytes segment `index` occupies in the file.
///
/// `bss` records a decompressed size but stores nothing, and the builder writes its `comp_size` as
/// zero to say so; every other segment stores exactly `comp_size` bytes.
fn stored_size(header: &Kip1Header, index: usize) -> usize {
    if index == BSS_INDEX {
        return 0;
    }

    header.segments[index].comp_size.get() as usize
}

/// Whether segment `index` is marked BLZ-packed by the header's flags.
///
/// Only bits `0`, `1`, and `2` are compression bits, so `bss` is never packed whatever bit `3`
/// holds — that bit selects a 64-bit process.
fn is_compressed(header: &Kip1Header, index: usize) -> bool {
    if index >= BSS_INDEX {
        return false;
    }

    header.flags & (1 << index) != 0
}

/// Errors that can occur when parsing a KIP1 from bytes
#[derive(Debug, thiserror::Error)]
pub enum FromBytesError {
    /// Buffer is too small to contain the KIP1 header
    #[error("buffer too small: need {required} bytes, have {available}")]
    BufferTooSmall {
        /// Number of bytes required
        required: usize,
        /// Number of bytes available
        available: usize,
    },
    /// Magic number does not match KIP1 (0x3150494b)
    #[error("invalid KIP1 magic: found {found:#010x}")]
    InvalidMagic {
        /// The magic that was found in place of `KIP1`
        found: u32,
    },
    /// A segment's offset and stored size overflow when added
    #[error("segment {segment_index} at {offset} of size {size} overflows")]
    SegmentBoundsOverflow {
        /// Index of the segment in the header
        segment_index: usize,
        /// Offset the segment starts at
        offset: usize,
        /// Bytes the segment occupies in the file
        size: usize,
    },
    /// A segment extends past the end of the image
    #[error(
        "segment {segment_index} at {offset} of size {size} runs past the {available} available"
    )]
    SegmentOutOfBounds {
        /// Index of the segment in the header
        segment_index: usize,
        /// Offset the segment starts at
        offset: usize,
        /// Bytes the segment occupies in the file
        size: usize,
        /// Number of bytes the image holds
        available: usize,
    },
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::{FromBytesError, Kip1};
    use crate::write::Kip1Builder;

    /// Build a KIP1 carrying the three segments, each holding a distinctive run.
    fn kip1(text: Vec<u8>, rodata: Vec<u8>, data: Vec<u8>) -> Vec<u8> {
        Kip1Builder::new()
            .name("roundtrip")
            .title_id(0x0100_0000_0000_1000)
            .text(text)
            .rodata(rodata)
            .data(data)
            .build()
            .expect("a small image should build")
    }

    #[test]
    fn try_from_bytes_with_a_built_image_succeeds() {
        //* Given
        let image = kip1(vec![0xAA; 0x400], vec![0xBB; 0x200], vec![0xCC; 0x100]);

        //* When
        let kip = Kip1::try_from_bytes(&image).expect("a built image should parse");

        //* Then
        assert_eq!(kip.name(), "roundtrip");
        assert_eq!(kip.title_id(), 0x0100_0000_0000_1000);
    }

    #[test]
    fn try_from_bytes_with_wrong_magic_fails() {
        //* Given
        let mut image = kip1(vec![0xAA; 0x100], vec![0xBB; 0x100], vec![0xCC; 0x100]);
        image[0] = b'X';

        //* When
        let result = Kip1::try_from_bytes(&image);

        //* Then
        assert!(matches!(result, Err(FromBytesError::InvalidMagic { .. })));
    }

    #[test]
    fn try_from_bytes_with_a_truncated_image_fails() {
        //* Given
        let image = kip1(vec![0xAA; 0x400], vec![0xBB; 0x200], vec![0xCC; 0x100]);
        let truncated = &image[..0x180];

        //* When
        let result = Kip1::try_from_bytes(truncated);

        //* Then
        assert!(matches!(
            result,
            Err(FromBytesError::SegmentOutOfBounds { .. })
        ));
    }

    #[test]
    fn decompress_with_a_built_image_returns_the_segments_that_were_packed() {
        //* Given
        // The whole point of the reader: what the builder compressed must come back unchanged.
        let text = vec![0xAA; 0x400];
        let rodata = vec![0xBB; 0x200];
        let data = vec![0xCC; 0x100];
        let image = kip1(text.clone(), rodata.clone(), data.clone());
        let kip = Kip1::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segments: Vec<Vec<u8>> = kip
            .segments()
            .map(|segment| segment.decompress().expect("a built segment should expand"))
            .collect();

        //* Then
        assert_eq!(segments, vec![text, rodata, data, Vec::new()]);
    }

    #[test]
    fn segment_with_the_bss_index_stores_no_bytes() {
        //* Given
        let image = kip1(vec![0xAA; 0x100], vec![0xBB; 0x100], vec![0xCC; 0x100]);
        let kip = Kip1::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let bss = kip.segment(3).expect("index 3 is the bss segment");

        //* Then
        assert!(bss.stored().is_empty(), "bss occupies no bytes in the file");
        assert!(!bss.is_compressed(), "bss has nothing to compress");
    }

    #[test]
    fn segment_with_an_index_past_the_loaded_segments_returns_none() {
        //* Given
        let image = kip1(vec![0xAA; 0x100], vec![0xBB; 0x100], vec![0xCC; 0x100]);
        let kip = Kip1::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let segment = kip.segment(4);

        //* Then
        assert!(segment.is_none());
    }

    #[test]
    fn is_compressed_with_a_built_image_marks_the_first_three_segments() {
        //* Given
        // The builder packs text, rodata, and data, and leaves bss alone.
        let image = kip1(vec![0xAA; 0x400], vec![0xBB; 0x200], vec![0xCC; 0x100]);
        let kip = Kip1::try_from_bytes(&image).expect("a built image should parse");

        //* When
        let flags: Vec<bool> = kip.segments().map(|s| s.is_compressed()).collect();

        //* Then
        assert_eq!(flags, vec![true, true, true, false]);
    }
}
