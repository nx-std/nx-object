//! ELF segment extraction for Nintendo Switch executables.

use std::vec::Vec;

use object::{
    Endianness, Object, ObjectSection, ObjectSegment,
    elf::{NT_GNU_BUILD_ID, PT_LOAD},
    read::elf::{ElfFile64, ProgramHeader},
};

use crate::write::{NroBuilder, NsoBuilder};

/// Information about an ELF section.
#[derive(Debug, Clone, Copy)]
pub struct SectionInfo {
    /// Virtual address of the section.
    pub addr: u64,
    /// Size of the section in bytes.
    pub size: u64,
}

/// Parsed ELF segments ready for NRO/NSO generation.
pub struct ElfSegments {
    text: Vec<u8>,
    text_vaddr: u64,
    rodata: Vec<u8>,
    rodata_vaddr: u64,
    data: Vec<u8>,
    data_vaddr: u64,
    bss_size: u64,
    build_id: Option<[u8; 0x20]>,
    mod0_offset: Option<u32>,
    #[allow(dead_code)]
    dynamic: Option<SectionInfo>,
    #[allow(dead_code)]
    dynstr: Option<SectionInfo>,
    #[allow(dead_code)]
    dynsym: Option<SectionInfo>,
    #[allow(dead_code)]
    eh_frame_hdr: Option<SectionInfo>,
}

impl ElfSegments {
    /// Parse an ELF file and extract segments for NRO/NSO generation.
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let elf = ElfFile64::<Endianness>::parse(data)?;

        // Verify architecture
        if elf.architecture() != object::Architecture::Aarch64 {
            return Err(ParseError::UnsupportedArch);
        }

        // Extract PT_LOAD segments
        // Store (address, data, memsz, filesz) for BSS size calculation
        // Need to access raw program headers to check p_type
        let endian = elf.endianness();
        let mut segments: Vec<_> = Vec::new();
        for (raw, seg) in elf.elf_program_headers().iter().zip(elf.segments()) {
            if raw.p_type(endian) == PT_LOAD {
                let memsz = seg.size();
                let filesz = seg.file_range().1;
                let data = seg
                    .data()
                    .map_err(|_| ParseError::UnreadableSegment(seg.address()))?;
                segments.push((seg.address(), data, memsz, filesz));
            }
        }

        // Sort by address
        segments.sort_by_key(|(addr, _, _, _)| *addr);

        // Extract text, rodata, data with their virtual addresses
        let (text_vaddr, text, _, _) = segments.first().ok_or(ParseError::MissingText)?;
        let text = text.to_vec();
        let text_vaddr = *text_vaddr;

        let (rodata_vaddr, rodata, _, _) = segments.get(1).ok_or(ParseError::MissingRodata)?;
        let rodata = rodata.to_vec();
        let rodata_vaddr = *rodata_vaddr;

        let (data_vaddr, data, _, _) = segments.get(2).ok_or(ParseError::MissingData)?;
        let data = data.to_vec();
        let data_vaddr = *data_vaddr;

        // Extract BSS size from optional 4th segment or from data segment memsz
        let bss_size = if segments.len() >= 4 {
            // 4-segment ELF: use the 4th PT_LOAD segment's memsz (not filesz/data length)
            // Pure BSS segments have filesz=0 but memsz>0, so seg.data().len() would be 0
            let memsz = segments.get(3).ok_or(ParseError::MissingData)?.2;
            (memsz + 0xFFF) & !0xFFF
        } else {
            // Check if data segment has memsz > filesz (embedded BSS)
            let (_addr, _data, memsz, filesz) = *segments.get(2).ok_or(ParseError::MissingData)?;
            if memsz > filesz {
                let aligned_filesz = (filesz + 0xFFF) & !0xFFF;
                if memsz > aligned_filesz {
                    ((memsz - aligned_filesz) + 0xFFF) & !0xFFF
                } else {
                    0
                }
            } else {
                0
            }
        };

        // Extract build ID from SHT_NOTE
        let build_id = extract_build_id(&elf);

        // Extract MOD0 offset from text segment
        let mod0_offset = if text.len() >= 8 {
            let offset_bytes: [u8; 4] = text[4..8].try_into().unwrap();
            let offset = u32::from_le_bytes(offset_bytes);
            if offset > 0 && offset < text.len() as u32 {
                Some(offset)
            } else {
                None
            }
        } else {
            None
        };

        // Extract section information
        let mut dynamic = None;
        let mut dynstr = None;
        let mut dynsym = None;
        let mut eh_frame_hdr = None;

        for section in elf.sections() {
            if let Ok(name) = section.name() {
                let info = SectionInfo {
                    addr: section.address(),
                    size: section.size(),
                };

                match name {
                    ".dynamic" => dynamic = Some(info),
                    ".dynstr" => dynstr = Some(info),
                    ".dynsym" => dynsym = Some(info),
                    ".eh_frame_hdr" => eh_frame_hdr = Some(info),
                    _ => {}
                }
            }
        }

        Ok(ElfSegments {
            text,
            text_vaddr,
            rodata,
            rodata_vaddr,
            data,
            data_vaddr,
            bss_size,
            build_id,
            mod0_offset,
            dynamic,
            dynstr,
            dynsym,
            eh_frame_hdr,
        })
    }

    /// Get the text (code) segment.
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// Get the rodata (read-only data) segment.
    pub fn rodata(&self) -> &[u8] {
        &self.rodata
    }

    /// Get the data (read-write data) segment.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get the BSS section size in bytes.
    pub fn bss_size(&self) -> u64 {
        self.bss_size
    }

    /// Get the 32-byte build ID, if present.
    pub fn build_id(&self) -> Option<&[u8; 0x20]> {
        self.build_id.as_ref()
    }

    /// Get the MOD0 offset relative to the start of the text segment.
    pub fn mod0_offset(&self) -> Option<u32> {
        self.mod0_offset
    }

    /// Convert into an NroBuilder with segments pre-populated.
    pub fn into_nro_builder(self) -> NroBuilder {
        let mut builder = NroBuilder::new()
            .text(self.text)
            .text_vaddr(self.text_vaddr as u32)
            .rodata(self.rodata)
            .rodata_vaddr(self.rodata_vaddr as u32)
            .data(self.data)
            .data_vaddr(self.data_vaddr as u32)
            .bss_size(self.bss_size as u32);

        if let Some(build_id) = self.build_id {
            builder = builder.build_id(build_id);
        }

        builder
    }

    /// Convert into an NsoBuilder with segments pre-populated.
    pub fn into_nso_builder(self) -> NsoBuilder {
        let mut builder = NsoBuilder::new()
            .text(self.text)
            .text_vaddr(self.text_vaddr as u32)
            .rodata(self.rodata)
            .rodata_vaddr(self.rodata_vaddr as u32)
            .data(self.data)
            .data_vaddr(self.data_vaddr as u32)
            .bss_size(self.bss_size as u32);

        if let Some(build_id) = self.build_id {
            builder = builder.module_id(build_id);
        }

        builder
    }

    /// Get the text segment virtual address.
    pub fn text_vaddr(&self) -> u64 {
        self.text_vaddr
    }

    /// Get the rodata segment virtual address.
    pub fn rodata_vaddr(&self) -> u64 {
        self.rodata_vaddr
    }

    /// Get the data segment virtual address.
    pub fn data_vaddr(&self) -> u64 {
        self.data_vaddr
    }
}

/// Error returned by [`ElfSegments::parse`].
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Invalid ELF file.
    #[error("invalid ELF: {0}")]
    InvalidElf(#[from] object::read::Error),
    /// Unsupported architecture (expected AArch64).
    #[error("unsupported architecture: expected AArch64")]
    UnsupportedArch,
    /// Missing text segment.
    #[error("missing text segment")]
    MissingText,
    /// Missing rodata segment.
    #[error("missing rodata segment")]
    MissingRodata,
    /// Missing data segment.
    #[error("missing data segment")]
    MissingData,
    /// PT_LOAD segment cannot be read.
    #[error("PT_LOAD segment at address {0:#x} cannot be read")]
    UnreadableSegment(u64),
}

/// Extract build ID from SHT_NOTE sections.
fn extract_build_id(elf: &ElfFile64<Endianness>) -> Option<[u8; 0x20]> {
    for section in elf.sections() {
        if section.kind() == object::SectionKind::Note
            && let Ok(data) = section.data()
        {
            // Parse note header: namesz (4), descsz (4), type (4), name, desc
            if data.len() < 12 {
                continue;
            }

            let namesz = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
            let descsz = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
            let note_type = u32::from_le_bytes(data[8..12].try_into().ok()?);

            // NT_GNU_BUILD_ID = 3
            // Validate: note type matches, name size is 4, and name is "GNU\0"
            if note_type == NT_GNU_BUILD_ID && namesz == 4 {
                // Validate we have enough data for note header + name
                if data.len() < 12 + namesz {
                    continue;
                }

                // Check name is "GNU\0"
                let name = &data[12..12 + namesz];
                if name != b"GNU\0" {
                    continue;
                }

                // Calculate descriptor offset with proper 4-byte alignment
                // Descriptor follows: nhdr (12 bytes) + aligned_name
                let desc_offset = 12 + ((namesz + 3) & !3);

                // Validate we have enough data for the descriptor
                if data.len() < desc_offset {
                    continue;
                }

                // Copy build ID, zero-padding if smaller than 0x20 bytes
                let mut build_id = [0u8; 0x20];
                let copy_size = descsz.min(0x20).min(data.len() - desc_offset);
                build_id[..copy_size].copy_from_slice(&data[desc_offset..desc_offset + copy_size]);
                return Some(build_id);
            }
        }
    }
    None
}
