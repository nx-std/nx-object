//! NRO (Nintendo Relocatable Object) builder.

use std::vec::Vec;

use zerocopy::FromZeros;

use crate::raw::{
    build_id::BuildId,
    nro::{ASSET_MAGIC, NRO_MAGIC, NroAssetHeader, NroHeader, NroSegment},
};

/// Builder for constructing NRO files.
pub struct NroBuilder {
    text: Option<Vec<u8>>,
    text_vaddr: u32,
    rodata: Option<Vec<u8>>,
    rodata_vaddr: u32,
    data: Option<Vec<u8>>,
    data_vaddr: u32,
    bss_size: u32,
    build_id: Option<BuildId>,
    flags: u32,
    icon: Option<Vec<u8>>,
    nacp: Option<Vec<u8>>,
    romfs: Option<Vec<u8>>,
}

impl NroBuilder {
    /// Create a new NRO builder.
    pub fn new() -> Self {
        Self {
            text: None,
            text_vaddr: 0,
            rodata: None,
            rodata_vaddr: 0,
            data: None,
            data_vaddr: 0,
            bss_size: 0,
            build_id: None,
            flags: 0,
            icon: None,
            nacp: None,
            romfs: None,
        }
    }

    /// Set the text (code) segment.
    pub fn text(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.text = Some(data.into());
        self
    }

    /// Set the text segment's ELF virtual address.
    pub fn text_vaddr(mut self, vaddr: u32) -> Self {
        self.text_vaddr = vaddr;
        self
    }

    /// Set the rodata (read-only data) segment.
    pub fn rodata(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.rodata = Some(data.into());
        self
    }

    /// Set the rodata segment's ELF virtual address.
    pub fn rodata_vaddr(mut self, vaddr: u32) -> Self {
        self.rodata_vaddr = vaddr;
        self
    }

    /// Set the data (read-write data) segment.
    pub fn data(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.data = Some(data.into());
        self
    }

    /// Set the data segment's ELF virtual address.
    pub fn data_vaddr(mut self, vaddr: u32) -> Self {
        self.data_vaddr = vaddr;
        self
    }

    /// Set the BSS section size in bytes.
    pub fn bss_size(mut self, size: u32) -> Self {
        self.bss_size = size;
        self
    }

    /// Set the 32-byte build ID.
    ///
    /// If not provided, will default to all zeros.
    pub fn build_id(mut self, id: BuildId) -> Self {
        self.build_id = Some(id);
        self
    }

    /// Set the NRO flags field.
    ///
    /// Bit 0: Aligned header layout (0x1)
    pub fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// Add an icon asset (JPEG image).
    pub fn asset_icon(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.icon = Some(data.into());
        self
    }

    /// Add a NACP (Nintendo Application Control Property) asset.
    pub fn asset_nacp(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.nacp = Some(data.into());
        self
    }

    /// Add a RomFS asset.
    pub fn asset_romfs(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.romfs = Some(data.into());
        self
    }

    /// Build the complete NRO file.
    pub fn build(self) -> Result<Vec<u8>, BuildError> {
        // Validate required fields
        let text = self.text.ok_or(BuildError::MissingText)?;
        let rodata = self.rodata.ok_or(BuildError::MissingRodata)?;
        let data = self.data.ok_or(BuildError::MissingData)?;

        // Pad segments to 0x1000 alignment
        let text_padded = pad_to_alignment(&text, 0x1000);
        let rodata_padded = pad_to_alignment(&rodata, 0x1000);
        let data_padded = pad_to_alignment(&data, 0x1000);

        // Calculate segment file offsets based on ELF vaddr layout.
        // Each segment's file offset equals its ELF virtual address, preserving the
        // ELF memory layout in the file. For typical homebrew ELFs with contiguous
        // segments starting at vaddr 0, this yields sequential offsets
        // (0x80, 0x80+text_len, 0x80+text_len+rodata_len). For ELFs with a non-zero
        // text base or vaddr gaps, segments are padded to preserve the vaddr layout.
        let text_offset = self.text_vaddr;
        let rodata_offset = self.rodata_vaddr;
        let data_offset = self.data_vaddr;

        // Calculate NRO size as the end of the last segment
        let nro_size = data_offset + data_padded.len() as u32;

        // Calculate total size including assets
        let has_assets = self.icon.is_some() || self.nacp.is_some() || self.romfs.is_some();
        let total_size = if has_assets {
            let icon_size = self.icon.as_ref().map_or(0, |v| v.len());
            let nacp_size = self.nacp.as_ref().map_or(0, |v| v.len());
            let romfs_size = self.romfs.as_ref().map_or(0, |v| v.len());
            nro_size as usize + 0x38 + icon_size + nacp_size + romfs_size
        } else {
            nro_size as usize
        };

        let mut buf = vec![0u8; total_size];

        // Write padded segments at their vaddr file offsets FIRST.
        // This preserves the ELF vaddr layout with padding between segments if needed.
        // For typical homebrew ELFs with text_vaddr=0, the text segment starts at offset 0,
        // and the header (written below) overwrites bytes 0x10..0x80 of the text region,
        // leaving the crt0-provided NroStart (0x00..0x10) intact.
        let text_start = text_offset as usize;
        let text_end = text_start + text_padded.len();
        buf[text_start..text_end].copy_from_slice(&text_padded);

        let rodata_start = rodata_offset as usize;
        let rodata_end = rodata_start + rodata_padded.len();
        buf[rodata_start..rodata_end].copy_from_slice(&rodata_padded);

        let data_start = data_offset as usize;
        let data_end = data_start + data_padded.len();
        buf[data_start..data_end].copy_from_slice(&data_padded);

        // The first 0x10 bytes (NroStart: entry branch instruction + MOD0 offset) belong
        // to the text segment's crt0 and were written above with the text segment. The
        // packer must NOT synthesize them — zeroing offset 0 destroys the entry branch
        // instruction, so the loader jumps into an invalid instruction and the NRO fails
        // to launch. Only the NroHeader at 0x10..0x80 is written here; bytes 0x00..0x10
        // are left exactly as the text segment provided them.

        // Write NroHeader at offset 0x10 (0x70 bytes)
        let mut header = NroHeader::new_zeroed();
        header.magic = NRO_MAGIC.into();
        header.version = 0.into();
        header.size = nro_size.into();
        header.flags = self.flags.into();
        header.segments = [
            NroSegment {
                file_off: text_offset.into(),
                size: (text_padded.len() as u32).into(),
            },
            NroSegment {
                file_off: rodata_offset.into(),
                size: (rodata_padded.len() as u32).into(),
            },
            NroSegment {
                file_off: data_offset.into(),
                size: (data_padded.len() as u32).into(),
            },
        ];
        header.bss_size = self.bss_size.into();
        header.build_id = self.build_id.unwrap_or([0u8; 0x20]);
        buf[0x10..0x80].copy_from_slice(zerocopy::IntoBytes::as_bytes(&header));

        // Write asset header and assets if present
        if has_assets {
            let asset_section_start = nro_size as usize;
            let mut asset_offset = 0x38u64; // Header size

            let (icon_off, icon_size) = if let Some(icon) = &self.icon {
                let off = asset_offset;
                asset_offset += icon.len() as u64;
                (off, icon.len() as u64)
            } else {
                (0, 0)
            };

            let (nacp_off, nacp_size) = if let Some(nacp) = &self.nacp {
                let off = asset_offset;
                asset_offset += nacp.len() as u64;
                (off, nacp.len() as u64)
            } else {
                (0, 0)
            };

            let (romfs_off, romfs_size) = if let Some(romfs) = &self.romfs {
                let off = asset_offset;
                (off, romfs.len() as u64)
            } else {
                (0, 0)
            };

            let asset_header = NroAssetHeader {
                magic: ASSET_MAGIC.into(),
                version: 0.into(),
                icon: crate::raw::nro::NroAssetSection {
                    offset: icon_off.into(),
                    size: icon_size.into(),
                },
                nacp: crate::raw::nro::NroAssetSection {
                    offset: nacp_off.into(),
                    size: nacp_size.into(),
                },
                romfs: crate::raw::nro::NroAssetSection {
                    offset: romfs_off.into(),
                    size: romfs_size.into(),
                },
            };

            // Write asset header at nro_size
            let header_start = asset_section_start;
            let header_end = header_start + 0x38;
            buf[header_start..header_end]
                .copy_from_slice(zerocopy::IntoBytes::as_bytes(&asset_header));

            // Write asset data after header
            let mut current_pos = header_end;
            if let Some(icon) = &self.icon {
                let end_pos = current_pos + icon.len();
                buf[current_pos..end_pos].copy_from_slice(icon);
                current_pos = end_pos;
            }
            if let Some(nacp) = &self.nacp {
                let end_pos = current_pos + nacp.len();
                buf[current_pos..end_pos].copy_from_slice(nacp);
                current_pos = end_pos;
            }
            if let Some(romfs) = &self.romfs {
                let end_pos = current_pos + romfs.len();
                buf[current_pos..end_pos].copy_from_slice(romfs);
            }
        }

        Ok(buf)
    }
}

impl Default for NroBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned by [`NroBuilder::build`].
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// `build` was called before a text segment was set.
    ///
    /// The text segment is mandatory; the builder cannot emit an NRO without it.
    #[error("missing text segment")]
    MissingText,
    /// `build` was called before a rodata segment was set.
    ///
    /// The rodata segment is mandatory; the builder cannot emit an NRO without it.
    #[error("missing rodata segment")]
    MissingRodata,
    /// `build` was called before a data segment was set.
    ///
    /// The data segment is mandatory; the builder cannot emit an NRO without it.
    #[error("missing data segment")]
    MissingData,
}

/// Pad a byte slice to the specified alignment.
fn pad_to_alignment(data: &[u8], alignment: usize) -> Vec<u8> {
    let len = data.len();
    let padded_len = len.div_ceil(alignment) * alignment;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(data);
    padded.resize(padded_len, 0);
    padded
}
