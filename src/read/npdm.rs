use zerocopy::FromBytes;

use crate::raw::npdm::{ACI0_MAGIC, ACID_MAGIC, Aci0Header, AcidHeader, META_MAGIC, NpdmHeader};

/// Errors that can occur when parsing NPDM from bytes
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
    /// META header magic does not match expected value (0x4154454d)
    #[error("invalid META magic: expected 0x4154454d (META), found {found:#010x}")]
    InvalidMetaMagic {
        /// Found magic number
        found: u32,
    },
    /// ACID header magic does not match expected value (0x44494341)
    #[error("invalid ACID magic: expected 0x44494341 (ACID), found {found:#010x}")]
    InvalidAcidMagic {
        /// Found magic number
        found: u32,
    },
    /// ACI0 header magic does not match expected value (0x30494341)
    #[error("invalid ACI0 magic: expected 0x30494341 (ACI0), found {found:#010x}")]
    InvalidAci0Magic {
        /// Found magic number
        found: u32,
    },
    /// ACID size field contains invalid value
    #[error("invalid ACID size: expected {}, found {found}", size_of::<AcidHeader>())]
    InvalidAcidSize {
        /// Found size value
        found: usize,
    },
    /// ACI0 size field contains invalid value
    #[error("invalid ACI0 size: expected {}, found {found}", size_of::<Aci0Header>())]
    InvalidAci0Size {
        /// Found size value
        found: usize,
    },
    /// ACI0 inner section offset+size exceeds buffer bounds
    ///
    /// This occurs when an ACI0 inner section (FAC, SAC, or KC) has an offset+size
    /// that extends beyond the NPDM buffer. A crafted NPDM can have valid ACI0 bounds
    /// but oversized inner section offset+size values.
    #[error(
        "ACI0 {section} section out of bounds: offset {offset} + size {size} = {end} > buffer size {buffer_size}"
    )]
    Aci0InnerSectionOutOfBounds {
        /// Section name ("FAC", "SAC", or "KC")
        section: &'static str,
        /// Section offset (relative to ACI0 start)
        offset: usize,
        /// Section size in bytes
        size: usize,
        /// Computed end position (offset + size)
        end: usize,
        /// Buffer size
        buffer_size: usize,
    },
}

/// High-level NPDM parser with access to META, ACID, and ACI0 sections.
pub struct Npdm<'a> {
    bytes: &'a [u8],
    header: &'a NpdmHeader,
    acid: &'a AcidHeader,
    aci0: &'a Aci0Header,
}

impl<'a> Npdm<'a> {
    /// Parse NPDM from bytes with magic and size validation.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        // Validate minimum size for META header
        if bytes.len() < size_of::<NpdmHeader>() {
            return Err(FromBytesError::BufferTooSmall {
                required: size_of::<NpdmHeader>(),
                available: bytes.len(),
            });
        }

        // Parse META header
        let header = NpdmHeader::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: 0x80,
                available: bytes.len(),
            })?
            .0;

        // Validate META magic
        if header.magic.get() != META_MAGIC {
            return Err(FromBytesError::InvalidMetaMagic {
                found: header.magic.get(),
            });
        }

        // Parse ACID at offset
        let acid_offset = header.acid_offset.get() as usize;
        let acid_size = header.acid_size.get() as usize;

        if acid_offset + acid_size > bytes.len() {
            return Err(FromBytesError::BufferTooSmall {
                required: acid_offset + acid_size,
                available: bytes.len(),
            });
        }

        if acid_size < size_of::<AcidHeader>() {
            return Err(FromBytesError::InvalidAcidSize { found: acid_size });
        }

        let acid = AcidHeader::ref_from_prefix(&bytes[acid_offset..])
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: acid_offset + size_of::<AcidHeader>(),
                available: bytes.len(),
            })?
            .0;

        // Validate ACID magic
        if acid.magic.get() != ACID_MAGIC {
            return Err(FromBytesError::InvalidAcidMagic {
                found: acid.magic.get(),
            });
        }

        // Parse ACI0 at offset
        let aci_offset = header.aci_offset.get() as usize;
        let aci_size = header.aci_size.get() as usize;

        if aci_offset + aci_size > bytes.len() {
            return Err(FromBytesError::BufferTooSmall {
                required: aci_offset + aci_size,
                available: bytes.len(),
            });
        }

        if aci_size < size_of::<Aci0Header>() {
            return Err(FromBytesError::InvalidAci0Size { found: aci_size });
        }

        let aci0 = Aci0Header::ref_from_prefix(&bytes[aci_offset..])
            .map_err(|_| FromBytesError::BufferTooSmall {
                required: aci_offset + size_of::<Aci0Header>(),
                available: bytes.len(),
            })?
            .0;

        // Validate ACI0 magic
        if aci0.magic.get() != ACI0_MAGIC {
            return Err(FromBytesError::InvalidAci0Magic {
                found: aci0.magic.get(),
            });
        }

        Ok(Self {
            bytes,
            header,
            acid,
            aci0,
        })
    }

    /// Get the META header
    pub fn header(&self) -> &NpdmHeader {
        self.header
    }

    /// Get program name (null-terminated UTF-8)
    pub fn name(&self) -> &str {
        let len = self
            .header
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.header.name.len());
        core::str::from_utf8(&self.header.name[..len]).unwrap_or("")
    }

    /// Get main thread priority (0-63)
    pub fn main_thread_priority(&self) -> u8 {
        self.header.main_thread_priority
    }

    /// Get main thread stack size
    pub fn main_thread_stack_size(&self) -> u32 {
        self.header.main_thread_stack_size.get()
    }

    /// Get the ACID header
    pub fn acid(&self) -> &AcidHeader {
        self.acid
    }

    /// Get allowed program ID range (min, max)
    pub fn program_id_range(&self) -> (u64, u64) {
        (
            self.acid.program_id_min.get(),
            self.acid.program_id_max.get(),
        )
    }

    /// Get the ACI0 header
    pub fn aci0(&self) -> &Aci0Header {
        self.aci0
    }

    /// Get program ID
    pub fn program_id(&self) -> u64 {
        self.aci0.program_id.get()
    }

    /// Get ACI0 Filesystem Access Control raw data
    ///
    /// # Errors
    ///
    /// Returns [`FromBytesError::Aci0InnerSectionOutOfBounds`] if the FAC section
    /// offset+size extends beyond the NPDM buffer bounds.
    pub fn aci0_fac_data(&self) -> Result<&'a [u8], FromBytesError> {
        let aci_offset = self.header.aci_offset.get() as usize;
        let fac_offset = self.aci0.fac_offset.get() as usize;
        let fac_size = self.aci0.fac_size.get() as usize;
        let start = aci_offset + fac_offset;
        let end =
            start
                .checked_add(fac_size)
                .ok_or(FromBytesError::Aci0InnerSectionOutOfBounds {
                    section: "FAC",
                    offset: fac_offset,
                    size: fac_size,
                    end: usize::MAX,
                    buffer_size: self.bytes.len(),
                })?;

        if end > self.bytes.len() {
            return Err(FromBytesError::Aci0InnerSectionOutOfBounds {
                section: "FAC",
                offset: fac_offset,
                size: fac_size,
                end,
                buffer_size: self.bytes.len(),
            });
        }

        Ok(&self.bytes[start..end])
    }

    /// Get ACI0 Service Access Control raw data
    ///
    /// # Errors
    ///
    /// Returns [`FromBytesError::Aci0InnerSectionOutOfBounds`] if the SAC section
    /// offset+size extends beyond the NPDM buffer bounds.
    pub fn aci0_sac_data(&self) -> Result<&'a [u8], FromBytesError> {
        let aci_offset = self.header.aci_offset.get() as usize;
        let sac_offset = self.aci0.sac_offset.get() as usize;
        let sac_size = self.aci0.sac_size.get() as usize;
        let start = aci_offset + sac_offset;
        let end =
            start
                .checked_add(sac_size)
                .ok_or(FromBytesError::Aci0InnerSectionOutOfBounds {
                    section: "SAC",
                    offset: sac_offset,
                    size: sac_size,
                    end: usize::MAX,
                    buffer_size: self.bytes.len(),
                })?;

        if end > self.bytes.len() {
            return Err(FromBytesError::Aci0InnerSectionOutOfBounds {
                section: "SAC",
                offset: sac_offset,
                size: sac_size,
                end,
                buffer_size: self.bytes.len(),
            });
        }

        Ok(&self.bytes[start..end])
    }

    /// Get ACI0 Kernel Capability raw data
    ///
    /// # Errors
    ///
    /// Returns [`FromBytesError::Aci0InnerSectionOutOfBounds`] if the KC section
    /// offset+size extends beyond the NPDM buffer bounds.
    pub fn aci0_kc_data(&self) -> Result<&'a [u8], FromBytesError> {
        let aci_offset = self.header.aci_offset.get() as usize;
        let kc_offset = self.aci0.kc_offset.get() as usize;
        let kc_size = self.aci0.kc_size.get() as usize;
        let start = aci_offset + kc_offset;
        let end =
            start
                .checked_add(kc_size)
                .ok_or(FromBytesError::Aci0InnerSectionOutOfBounds {
                    section: "KC",
                    offset: kc_offset,
                    size: kc_size,
                    end: usize::MAX,
                    buffer_size: self.bytes.len(),
                })?;

        if end > self.bytes.len() {
            return Err(FromBytesError::Aci0InnerSectionOutOfBounds {
                section: "KC",
                offset: kc_offset,
                size: kc_size,
                end,
                buffer_size: self.bytes.len(),
            });
        }

        Ok(&self.bytes[start..end])
    }
}
