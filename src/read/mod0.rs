//! Validated reader over an embedded MOD0 header.
//!
//! [`Mod0::try_from_bytes`] checks the magic and the length before borrowing, so
//! every accessor past it reads a header known to be present and complete.
//!
//! The offsets it returns are signed and relative to the MOD0 header's own
//! position, so resolving one means adding it to where the header was found —
//! this reader does not know that address and cannot do it for the caller.

use zerocopy::FromBytes;

use crate::raw::mod0::{MOD0_MAGIC, Mod0Header};

/// A borrowed view of a MOD0 header found inside an executable image.
///
/// Every offset it exposes is signed and measured from where the header sits, so a caller
/// resolving one has to add the header's own address back. This type never saw that address, which
/// is why it hands the offsets over as they are stored rather than resolving them.
pub struct Mod0<'a> {
    header: &'a Mod0Header,
}

impl<'a> Mod0<'a> {
    /// Validate `bytes` as a MOD0 header and borrow it.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is shorter than the header or if the magic
    /// does not match — the latter usually meaning the MOD0 offset taken from the
    /// enclosing image pointed somewhere else.
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

    /// Read a MOD0 header already mapped in memory, at an address the caller resolved.
    ///
    /// The header is fixed-size, so exactly its length is borrowed.
    ///
    /// # Safety
    ///
    /// - `ptr` points at `0x1C` readable bytes in one allocation.
    /// - That mapping stays live and unwritten for `'a`.
    ///
    /// # Errors
    ///
    /// Fails when the magic is not `MOD0`, which means the offset used to reach this address did
    /// not point at a MOD0 header. The length check cannot fail, since the slice is built at
    /// exactly the required length.
    pub unsafe fn try_from_ptr(ptr: *const u8) -> Result<Self, FromPtrError> {
        // SAFETY: Caller guarantees ptr is valid and memory remains valid for 'a
        let bytes = unsafe { core::slice::from_raw_parts(ptr, size_of::<Mod0Header>()) };
        Self::try_from_bytes(bytes).map_err(FromPtrError)
    }

    /// The header itself, for the fields these accessors do not name.
    pub fn header(&self) -> &Mod0Header {
        self.header
    }

    /// Where the `.dynamic` section sits, relative to the header.
    pub fn dynamic_offset(&self) -> i32 {
        self.header.dynamic_offset.get()
    }

    /// Where BSS begins, relative to the header.
    pub fn bss_start_offset(&self) -> i32 {
        self.header.bss_start_offset.get()
    }

    /// Where BSS ends, relative to the header, one past its last byte.
    pub fn bss_end_offset(&self) -> i32 {
        self.header.bss_end_offset.get()
    }

    /// Where `.eh_frame_hdr` begins, relative to the header.
    pub fn eh_frame_hdr_start(&self) -> i32 {
        self.header.eh_frame_hdr_start.get()
    }

    /// Where `.eh_frame_hdr` ends, relative to the header, one past its last byte.
    pub fn eh_frame_hdr_end(&self) -> i32 {
        self.header.eh_frame_hdr_end.get()
    }

    /// Where the runtime's module object sits, relative to the header.
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
