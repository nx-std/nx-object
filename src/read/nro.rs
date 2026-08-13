//! Validated reader over an NRO image and its optional asset section.
//!
//! [`Nro::try_from_bytes`] checks the magic and proves every segment lies inside
//! the buffer before borrowing it, so the segment accessors return slices without
//! re-checking and cannot panic on a malformed image.
//!
//! The asset section is optional and detected by its own magic past the end of the
//! executable, so the asset accessors return `None` for a bare NRO rather than
//! failing.

use zerocopy::FromBytes;

use crate::raw::nro::{
    ASSET_MAGIC, NRO_MAGIC, NroAssetHeader, NroAssetSection, NroHeader, NroStart,
};

/// A borrowed view of an NRO image whose segment and asset bounds have already been checked.
///
/// Construction is where every check happens, so an `Nro` that exists is one whose segments lie
/// inside the buffer. That is what lets the accessors below return slices directly, with no
/// `Result` and no possibility of panicking on a crafted image.
pub struct Nro<'a> {
    bytes: &'a [u8],
    start: &'a NroStart,
    header: &'a NroHeader,
    asset_header: Option<&'a NroAssetHeader>,
}

impl<'a> Nro<'a> {
    /// Validate `bytes` as an NRO and borrow it, proving every segment lies inside the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot hold the start stub and header, if
    /// the magic does not match, or if any segment's offset and size overflow or
    /// run past the end of the buffer. A missing asset section is not an error:
    /// the asset accessors return `None` for a bare NRO.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        // Validate minimum size for start + header
        let min_size = size_of::<NroStart>() + size_of::<NroHeader>();
        if bytes.len() < min_size {
            return Err(FromBytesError::BufferTooSmall {
                required: min_size,
                available: bytes.len(),
            });
        }

        let start = NroStart::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x10,
                available: bytes.len(),
            })?
            .0;

        let header = NroHeader::ref_from_prefix(&bytes[0x10..])
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x80,
                available: bytes.len(),
            })?
            .0;

        // Validate magic
        if header.magic.get() != NRO_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        // Validate segment bounds
        for (idx, seg) in header.segments.iter().enumerate() {
            let off = seg.file_off.get() as usize;
            let size = seg.size.get() as usize;
            let end = off
                .checked_add(size)
                .ok_or(FromBytesError::SegmentBoundsOverflow {
                    segment_index: idx,
                    offset: off,
                    size,
                })?;
            if end > bytes.len() {
                return Err(FromBytesError::SegmentOutOfBounds {
                    segment_index: idx,
                    offset: off,
                    size,
                    available: bytes.len(),
                });
            }
        }

        // Try to parse asset header (at end of NRO)
        let nro_size = header.size.get() as usize;
        let asset_header = if bytes.len() > nro_size + size_of::<NroAssetHeader>() {
            NroAssetHeader::ref_from_prefix(&bytes[nro_size..])
                .ok()
                .map(|(h, _)| h)
                .filter(|h| h.magic.get() == ASSET_MAGIC)
        } else {
            None
        };

        // Validate asset section bounds if asset header exists
        if let Some(asset_hdr) = asset_header {
            let base = nro_size;
            for (name, section) in [
                ("icon", &asset_hdr.icon),
                ("nacp", &asset_hdr.nacp),
                ("romfs", &asset_hdr.romfs),
            ] {
                let off = section.offset.get() as usize;
                let size = section.size.get() as usize;
                if size == 0 {
                    continue; // Empty sections are valid
                }
                let abs_off =
                    base.checked_add(off)
                        .ok_or(FromBytesError::AssetSectionBoundsOverflow {
                            section_name: name,
                            base,
                            offset: off,
                            size,
                        })?;
                let end = abs_off.checked_add(size).ok_or(
                    FromBytesError::AssetSectionBoundsOverflow {
                        section_name: name,
                        base,
                        offset: off,
                        size,
                    },
                )?;
                if end > bytes.len() {
                    return Err(FromBytesError::AssetSectionOutOfBounds {
                        section_name: name,
                        offset: abs_off,
                        size,
                        available: bytes.len(),
                    });
                }
            }
        }

        Ok(Self {
            bytes,
            start,
            header,
            asset_header,
        })
    }

    /// The entry stub occupying the first `0x10` bytes, which the loader jumps to.
    pub fn start(&self) -> &NroStart {
        self.start
    }

    /// The header describing the image's extent, segments, and identity.
    pub fn header(&self) -> &NroHeader {
        self.header
    }

    /// The asset header, or `None` for a bare NRO carrying no icon, NACP, or RomFS.
    pub fn asset_header(&self) -> Option<&NroAssetHeader> {
        self.asset_header
    }

    /// Identity of the build this image was linked from.
    pub fn build_id(&self) -> &[u8; 32] {
        &self.header.build_id
    }

    /// The executable code the loader maps.
    pub fn text_segment(&self) -> &[u8] {
        self.segment(0)
    }

    /// The constants and dynamic linking tables the loader maps read-only.
    pub fn rodata_segment(&self) -> &[u8] {
        self.segment(1)
    }

    /// The writable initialized data the loader maps.
    pub fn data_segment(&self) -> &[u8] {
        self.segment(2)
    }

    // Indexing and slicing are unchecked on purpose: `try_from_bytes` proved every segment's
    // offset and size fit the buffer, and `idx` comes from the three callers above rather than
    // from the image.
    fn segment(&self, idx: usize) -> &[u8] {
        let seg = &self.header.segments[idx];
        let off = seg.file_off.get() as usize;
        let size = seg.size.get() as usize;
        &self.bytes[off..off + size]
    }

    /// The JPEG the homebrew menu displays, or `None` when the image carries no icon.
    pub fn asset_icon(&self) -> Option<&'a [u8]> {
        self.asset_section(|h| &h.icon)
    }

    /// The NACP supplying title and author, or `None` when the image carries none.
    pub fn asset_nacp(&self) -> Option<&'a [u8]> {
        self.asset_section(|h| &h.nacp)
    }

    /// The RomFS image the program mounts, or `None` when the image carries none.
    pub fn asset_romfs(&self) -> Option<&'a [u8]> {
        self.asset_section(|h| &h.romfs)
    }

    fn asset_section<F>(&self, f: F) -> Option<&'a [u8]>
    where
        F: FnOnce(&NroAssetHeader) -> &NroAssetSection,
    {
        let header = self.asset_header?;
        let section = f(header);
        let base = self.header.size.get() as usize;
        let off = base + section.offset.get() as usize;
        let size = section.size.get() as usize;
        if size == 0 {
            return None;
        }
        Some(&self.bytes[off..off + size])
    }

    /// Read an NRO already mapped in memory, for a program inspecting its own image.
    ///
    /// The length is not known in advance, so the pointer is treated as the start of an
    /// effectively unbounded slice and the header's own extents decide what is read. Every bound
    /// the buffer would otherwise impose comes from the image itself, which is why the caller has
    /// to guarantee the mapping really is an NRO.
    ///
    /// # Safety
    ///
    /// - `ptr` points at a mapped NRO image, in one allocation with the segments it describes.
    /// - That mapping stays live and unwritten for `'a`.
    ///
    /// # Errors
    ///
    /// Fails on the same conditions as [`Nro::try_from_bytes`]: a magic that is not `NRO0`, or a
    /// segment whose offset and size overflow. A bounds failure against the end of the buffer
    /// cannot be reported here, because the length the pointer stands for is not real.
    pub unsafe fn try_from_ptr(ptr: *const u8) -> Result<Self, FromPtrError> {
        // Create slice from pointer - we don't know size yet, use max reasonable
        // SAFETY: Caller guarantees ptr is valid and memory remains valid for 'a
        let bytes = unsafe { core::slice::from_raw_parts(ptr, usize::MAX / 2) };
        Self::try_from_bytes(bytes).map_err(FromPtrError)
    }
}

/// Errors that can occur when parsing an NRO from bytes
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
    /// Magic number does not match NRO0 (0x304f524e)
    #[error("invalid magic: expected 0x304f524e (NRO0), found {found:#010x}")]
    InvalidMagic {
        /// Found magic number
        found: u32,
    },
    /// Segment descriptor offset + size causes integer overflow
    ///
    /// This occurs when the segment's file offset and size, when added together,
    /// exceed the maximum value of usize. This indicates a malformed or crafted
    /// NRO file with invalid segment descriptors.
    #[error("segment {segment_index} offset + size overflow: offset={offset:#x}, size={size:#x}")]
    SegmentBoundsOverflow {
        /// Segment index (0=text, 1=rodata, 2=data)
        segment_index: usize,
        /// Segment file offset
        offset: usize,
        /// Segment size
        size: usize,
    },
    /// Segment descriptor points outside the buffer
    ///
    /// This occurs when a segment's file offset and size extend beyond the
    /// available buffer. Crafted NRO files with out-of-bounds segment descriptors
    /// will produce this error instead of panicking.
    #[error(
        "segment {segment_index} out of bounds: offset={offset:#x}, size={size:#x}, available={available:#x}"
    )]
    SegmentOutOfBounds {
        /// Segment index (0=text, 1=rodata, 2=data)
        segment_index: usize,
        /// Segment file offset
        offset: usize,
        /// Segment size
        size: usize,
        /// Available buffer size
        available: usize,
    },
    /// Asset section descriptor offset + size causes integer overflow
    ///
    /// This occurs when computing the absolute offset of an asset section
    /// (base + offset + size) causes integer overflow. This indicates a
    /// malformed or crafted NRO file with invalid asset section descriptors.
    #[error(
        "asset section '{section_name}' offset calculation overflow: base={base:#x}, offset={offset:#x}, size={size:#x}"
    )]
    AssetSectionBoundsOverflow {
        /// Asset section name (icon, nacp, or romfs)
        section_name: &'static str,
        /// Base offset (NRO size)
        base: usize,
        /// Section relative offset
        offset: usize,
        /// Section size
        size: usize,
    },
    /// Asset section descriptor points outside the buffer
    ///
    /// This occurs when an asset section's absolute offset and size extend
    /// beyond the available buffer. Crafted NRO files with out-of-bounds
    /// asset section descriptors will produce this error instead of panicking.
    #[error(
        "asset section '{section_name}' out of bounds: offset={offset:#x}, size={size:#x}, available={available:#x}"
    )]
    AssetSectionOutOfBounds {
        /// Asset section name (icon, nacp, or romfs)
        section_name: &'static str,
        /// Absolute file offset
        offset: usize,
        /// Section size
        size: usize,
        /// Available buffer size
        available: usize,
    },
}

/// Error when parsing NRO from raw pointer
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct FromPtrError(FromBytesError);
