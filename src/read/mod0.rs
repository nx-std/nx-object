#![expect(dead_code)]

use zerocopy::FromBytes;

use crate::raw::mod0::{MOD0_MAGIC, Mod0Header};

/// High-level MOD0 parser with offset accessors.
pub struct Mod0<'a> {
    header: &'a Mod0Header,
}

impl<'a> Mod0<'a> {
    /// Parse MOD0 from bytes with magic validation.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        if bytes.len() < size_of::<Mod0Header>() {
            return Err(FromBytesError::BufferTooSmall {
                required: size_of::<Mod0Header>(),
                available: bytes.len(),
            });
        }

        let header = Mod0Header::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x1C,
                available: bytes.len(),
            })?
            .0;

        if header.magic.get() != MOD0_MAGIC {
            return Err(FromBytesError::InvalidMagic {
                found: header.magic.get(),
            });
        }

        Ok(Self { header })
    }

    /// Create from raw pointer
    ///
    /// # Safety
    /// - `ptr` must point to valid MOD0 data
    /// - The memory must remain valid for lifetime `'a`
    pub unsafe fn try_from_ptr(ptr: *const u8) -> Result<Self, FromPtrError> {
        // SAFETY: Caller guarantees ptr is valid and memory remains valid for 'a
        let bytes = unsafe { core::slice::from_raw_parts(ptr, size_of::<Mod0Header>()) };
        Self::try_from_bytes(bytes).map_err(FromPtrError)
    }

    /// Get the MOD0 header.
    pub fn header(&self) -> &Mod0Header {
        self.header
    }

    /// Get offset to .dynamic section.
    pub fn dynamic_offset(&self) -> i32 {
        self.header.dynamic_offset.get()
    }

    /// Get offset to BSS start.
    pub fn bss_start_offset(&self) -> i32 {
        self.header.bss_start_offset.get()
    }

    /// Get offset to BSS end.
    pub fn bss_end_offset(&self) -> i32 {
        self.header.bss_end_offset.get()
    }

    /// Get offset to .eh_frame_hdr start.
    pub fn eh_frame_hdr_start(&self) -> i32 {
        self.header.eh_frame_hdr_start.get()
    }

    /// Get offset to .eh_frame_hdr end.
    pub fn eh_frame_hdr_end(&self) -> i32 {
        self.header.eh_frame_hdr_end.get()
    }

    /// Get offset to module object.
    pub fn module_object_offset(&self) -> i32 {
        self.header.module_object_offset.get()
    }
}

/// Errors that can occur when parsing MOD0 from bytes
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
    /// Magic number does not match MOD0 (0x30444f4d)
    #[error("invalid magic: expected 0x30444f4d (MOD0), found {found:#010x}")]
    InvalidMagic {
        /// Found magic number
        found: u32,
    },
}

/// Error when parsing MOD0 from raw pointer
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct FromPtrError(FromBytesError);
