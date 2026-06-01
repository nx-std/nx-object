//! KIP1 (Kernel Initial Process) builder.
//!
//! KIP1 is the format for system modules that execute in kernel mode.
//! This module provides a builder for creating valid KIP1 files.
//!
//! # BLZ Compression
//!
//! KIP1 segments (text, rodata, data) are compressed using the BLZ algorithm,
//! implemented in the crate-private `blz` module.

use std::vec::Vec;

use zerocopy::FromZeros;

use crate::{
    blz,
    raw::kip::{KIP1_MAGIC, Kip1Header, Kip1Segment},
};

/// Builder for constructing KIP1 files.
pub struct Kip1Builder {
    name: Option<String>,
    title_id: u64,
    process_category: u32,
    main_thread_priority: u8,
    default_cpu_id: u8,
    flags: Option<u8>,
    text: Option<Vec<u8>>,
    text_vaddr: u32,
    rodata: Option<Vec<u8>>,
    rodata_vaddr: u32,
    rodata_attributes: u32,
    data: Option<Vec<u8>>,
    data_vaddr: u32,
    bss_vaddr: Option<u32>,
    bss_size: Option<u32>,
    kernel_capabilities: Vec<u32>,
}

impl Kip1Builder {
    /// Create a new KIP1 builder.
    pub fn new() -> Self {
        Self {
            name: None,
            title_id: 0,
            process_category: 0,
            main_thread_priority: 0,
            default_cpu_id: 0,
            flags: None,
            text: None,
            text_vaddr: 0,
            rodata: None,
            rodata_vaddr: 0,
            rodata_attributes: 0,
            data: None,
            data_vaddr: 0,
            bss_vaddr: None,
            bss_size: None,
            kernel_capabilities: Vec::new(),
        }
    }

    /// Set the process name (up to 12 bytes, null-terminated).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the title ID.
    pub fn title_id(mut self, id: u64) -> Self {
        self.title_id = id;
        self
    }

    /// Set the process category.
    pub fn process_category(mut self, category: u32) -> Self {
        self.process_category = category;
        self
    }

    /// Set the main thread priority (0-63).
    pub fn main_thread_priority(mut self, priority: u8) -> Self {
        self.main_thread_priority = priority;
        self
    }

    /// Set the default CPU core ID.
    pub fn default_cpu_id(mut self, cpu_id: u8) -> Self {
        self.default_cpu_id = cpu_id;
        self
    }

    /// Set the flags byte manually.
    ///
    /// - Bit 0-2: Compression flags (text, rodata, data)
    /// - Bit 3: Is64Bit
    /// - Bit 4: IsAddrSpace32Bit
    /// - Bit 5: UseSystemPoolPartition
    /// - Bit 6: Immortal (process cannot be terminated by kernel)
    ///
    /// # Default Behavior
    ///
    /// If not set, the builder defaults to `0x3F` (`0b0011_1111`) for AArch64:
    /// bits 0-5 set, with bit 6 (Immortal) left clear.
    ///
    /// The Immortal flag controls whether the kernel may terminate the process. It is
    /// typically set for system modules but is not required for most homebrew use cases.
    pub fn flags(mut self, flags: u8) -> Self {
        self.flags = Some(flags);
        self
    }

    /// Set the text (code) segment.
    pub fn text(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.text = Some(data.into());
        self
    }

    /// Set the text segment's virtual address.
    pub fn text_vaddr(mut self, vaddr: u32) -> Self {
        self.text_vaddr = vaddr;
        self
    }

    /// Set the rodata (read-only data) segment.
    pub fn rodata(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.rodata = Some(data.into());
        self
    }

    /// Set the rodata segment's virtual address.
    pub fn rodata_vaddr(mut self, vaddr: u32) -> Self {
        self.rodata_vaddr = vaddr;
        self
    }

    /// Set the rodata segment attributes (e.g., main thread stack size).
    pub fn rodata_attributes(mut self, attributes: u32) -> Self {
        self.rodata_attributes = attributes;
        self
    }

    /// Set the data (read-write data) segment.
    pub fn data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Set the data segment's virtual address.
    pub fn data_vaddr(mut self, vaddr: u32) -> Self {
        self.data_vaddr = vaddr;
        self
    }

    /// Set the BSS virtual address.
    ///
    /// **IMPORTANT**: Both `bss_vaddr` and `bss_size` must be set together.
    /// Setting only one will result in a `BuildError::IncompleteBssConfiguration`.
    pub fn bss_vaddr(mut self, vaddr: u32) -> Self {
        self.bss_vaddr = Some(vaddr);
        self
    }

    /// Set the BSS size in bytes.
    ///
    /// **IMPORTANT**: Both `bss_vaddr` and `bss_size` must be set together.
    /// Setting only one will result in a `BuildError::IncompleteBssConfiguration`.
    pub fn bss_size(mut self, size: u32) -> Self {
        self.bss_size = Some(size);
        self
    }

    /// Add kernel capability descriptors (pre-encoded).
    ///
    /// Each u32 value should be an encoded kernel capability descriptor.
    /// See `write::npdm::KernelCapability::encode()` for encoding helpers.
    pub fn kernel_capabilities(mut self, capabilities: impl IntoIterator<Item = u32>) -> Self {
        self.kernel_capabilities.extend(capabilities);
        self
    }

    /// Build the complete KIP1 file.
    ///
    /// # Errors
    ///
    /// Returns `BuildError` if:
    /// - Required segments (text, rodata, data) are missing
    /// - Process name is missing
    /// - BSS configuration is incomplete (only one of `bss_vaddr` or `bss_size` is set)
    /// - Kernel capabilities exceed 0x20 entries (32 u32 values)
    pub fn build(self) -> Result<Vec<u8>, BuildError> {
        // Validate required fields
        let name = self.name.ok_or(BuildError::MissingName)?;
        let text = self.text.ok_or(BuildError::MissingText)?;
        let rodata = self.rodata.ok_or(BuildError::MissingRodata)?;
        let data = self.data.ok_or(BuildError::MissingData)?;

        // Validate BSS configuration completeness
        match (self.bss_vaddr, self.bss_size) {
            (Some(_), None) => {
                return Err(BuildError::IncompleteBssConfiguration {
                    has_vaddr: true,
                    has_size: false,
                });
            }
            (None, Some(_)) => {
                return Err(BuildError::IncompleteBssConfiguration {
                    has_vaddr: false,
                    has_size: true,
                });
            }
            _ => {} // Both set or both None - valid configurations
        }

        // Validate kernel capabilities count
        if self.kernel_capabilities.len() > 0x20 {
            return Err(BuildError::TooManyKernelCapabilities {
                count: self.kernel_capabilities.len(),
            });
        }

        // Compress segments
        let text_uncompressed_len = text.len();
        let text_compressed = blz::compress(&text);
        let rodata_uncompressed_len = rodata.len();
        let rodata_compressed = blz::compress(&rodata);
        let data_uncompressed_len = data.len();
        let data_compressed = blz::compress(&data);

        // Compute flags if not explicitly set
        let flags = self.flags.unwrap_or({
            // Default: 0x3F — bits 0-5 set, with Immortal (bit 6) left clear
            // Bits 0-2: compression enabled (text, rodata, data)
            // Bit 3: Is64Bit
            // Bit 4: IsAddrSpace32Bit
            // Bit 5: UseSystemPoolPartition
            // Bit 6: Immortal (left clear by default)
            0b0011_1111
        });

        // Build header
        let mut header = Kip1Header::new_zeroed();
        header.magic = KIP1_MAGIC.into();

        // Process name (12 bytes, null-terminated)
        // Truncate to 11 bytes max to ensure 12th byte remains null terminator
        let mut name_bytes = name.into_bytes();
        name_bytes.truncate(11);
        name_bytes.resize(12, 0);
        header.name.copy_from_slice(&name_bytes);

        header.title_id = self.title_id.into();
        header.process_category = self.process_category.into();
        header.main_thread_priority = self.main_thread_priority;
        header.default_cpu_id = self.default_cpu_id;
        header.flags = flags;

        // Text segment
        header.segments[0] = Kip1Segment {
            dst_addr: self.text_vaddr.into(),
            decomp_size: (text_uncompressed_len as u32).into(),
            comp_size: (text_compressed.len() as u32).into(),
            attributes: 0u32.into(),
        };

        // Rodata segment
        header.segments[1] = Kip1Segment {
            dst_addr: self.rodata_vaddr.into(),
            decomp_size: (rodata_uncompressed_len as u32).into(),
            comp_size: (rodata_compressed.len() as u32).into(),
            attributes: self.rodata_attributes.into(),
        };

        // Data segment
        header.segments[2] = Kip1Segment {
            dst_addr: self.data_vaddr.into(),
            decomp_size: (data_uncompressed_len as u32).into(),
            comp_size: (data_compressed.len() as u32).into(),
            attributes: 0u32.into(),
        };

        // BSS segment (if provided)
        if let (Some(bss_vaddr), Some(bss_size)) = (self.bss_vaddr, self.bss_size) {
            header.segments[3] = Kip1Segment {
                dst_addr: bss_vaddr.into(),
                decomp_size: bss_size.into(),
                comp_size: 0u32.into(),
                attributes: 0u32.into(),
            };
        }

        // Segments 4-5 remain zeroed (reserved)

        // Kernel capabilities (0x80 bytes = 32 u32 values)
        let cap_bytes = {
            let mut bytes = vec![0xFFu8; 0x80];
            for (i, cap) in self.kernel_capabilities.iter().enumerate() {
                let offset = i * 4;
                bytes[offset..offset + 4].copy_from_slice(&cap.to_le_bytes());
            }
            // Remaining bytes stay 0xFF (padding)
            bytes
        };
        header.capabilities.copy_from_slice(&cap_bytes);

        // Serialize header + compressed segments
        let header_bytes = zerocopy::IntoBytes::as_bytes(&header);
        let total_len = header_bytes.len()
            + text_compressed.len()
            + rodata_compressed.len()
            + data_compressed.len();

        let mut output = Vec::with_capacity(total_len);
        output.extend_from_slice(header_bytes);
        output.extend_from_slice(&text_compressed);
        output.extend_from_slice(&rodata_compressed);
        output.extend_from_slice(&data_compressed);

        Ok(output)
    }
}

impl Default for Kip1Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when building a KIP1 file.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// `build` was called before the process name was set.
    ///
    /// The process name is a mandatory field of the KIP1 header.
    #[error("missing process name")]
    MissingName,
    /// `build` was called before a text segment was set.
    ///
    /// The text segment is mandatory; the builder cannot emit a KIP1 without it.
    #[error("missing text segment")]
    MissingText,
    /// `build` was called before a rodata segment was set.
    ///
    /// The rodata segment is mandatory; the builder cannot emit a KIP1 without it.
    #[error("missing rodata segment")]
    MissingRodata,
    /// `build` was called before a data segment was set.
    ///
    /// The data segment is mandatory; the builder cannot emit a KIP1 without it.
    #[error("missing data segment")]
    MissingData,
    /// BSS configuration is incomplete
    ///
    /// Both `bss_vaddr` and `bss_size` must be set together, or both must be unset.
    /// Setting only one creates an invalid BSS segment configuration.
    ///
    /// This error prevents silent omission of BSS segments when a caller sets only one
    /// BSS parameter without the other.
    #[error(
        "incomplete BSS configuration: bss_vaddr={has_vaddr}, bss_size={has_size} (both must be set together or both unset)"
    )]
    IncompleteBssConfiguration {
        /// Whether `bss_vaddr` was set.
        has_vaddr: bool,
        /// Whether `bss_size` was set.
        has_size: bool,
    },
    /// Too many kernel capability descriptors (max 32).
    #[error("too many kernel capabilities: {count} (max 32)")]
    TooManyKernelCapabilities {
        /// The number of capabilities provided.
        count: usize,
    },
}
