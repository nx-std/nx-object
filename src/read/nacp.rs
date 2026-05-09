use zerocopy::FromBytes;

use crate::raw::nacp::{NacpLanguageEntry, NacpStruct};

/// High-level NACP parser with language entry access.
pub struct Nacp<'a> {
    raw: &'a NacpStruct,
}

impl<'a> Nacp<'a> {
    /// Parse NACP from bytes with validation.
    pub fn try_from_bytes(bytes: &'a [u8]) -> Result<Self, FromBytesError> {
        if bytes.len() < size_of::<NacpStruct>() {
            return Err(FromBytesError {
                required: size_of::<NacpStruct>(),
                available: bytes.len(),
            });
        }
        let raw = NacpStruct::ref_from_prefix(bytes)
            .map_err(|_| FromBytesError {
                required: 0x4000,
                available: bytes.len(),
            })?
            .0;
        Ok(Self { raw })
    }

    /// Get raw NACP structure.
    pub fn raw(&self) -> &NacpStruct {
        self.raw
    }

    /// Get language entry by index (0-15), or None if empty/invalid.
    pub fn language_entry(&self, index: usize) -> Option<&NacpLanguageEntry> {
        if index >= 16 {
            return None;
        }
        let entry = &self.raw.lang[index];
        // Check if entry is empty (first byte of name is null)
        if entry.name[0] == 0 {
            return None;
        }
        Some(entry)
    }

    /// Get display version string.
    pub fn display_version(&self) -> &str {
        cstr_to_str(&self.raw.display_version)
    }

    /// Get language entry for a specific language with fallback to first available.
    pub fn language_entry_for(&self, lang: SetLanguage) -> Option<&NacpLanguageEntry> {
        let idx = LANGUAGE_TABLE.get(lang as usize).copied().unwrap_or(0);

        // Try requested language
        if let Some(entry) = self.language_entry(idx) {
            return Some(entry);
        }

        // Fallback: find first non-empty entry
        for i in 0..16 {
            if let Some(entry) = self.language_entry(i) {
                return Some(entry);
            }
        }

        None
    }

    /// Create from raw pointer
    ///
    /// # Safety
    /// - `ptr` must point to valid NACP data (0x4000 bytes)
    /// - The memory must remain valid for lifetime `'a`
    pub unsafe fn try_from_ptr(ptr: *const u8) -> Result<Self, FromPtrError> {
        // SAFETY: Caller guarantees ptr is valid and memory remains valid for 'a
        let bytes = unsafe { core::slice::from_raw_parts(ptr, 0x4000) };
        Self::try_from_bytes(bytes).map_err(FromPtrError)
    }
}

/// Error when parsing NACP: buffer is too small
#[derive(Debug, thiserror::Error)]
#[error("buffer too small: need {required} bytes, have {available}")]
pub struct FromBytesError {
    /// Number of bytes required
    pub required: usize,
    /// Number of bytes available
    pub available: usize,
}

/// Error when parsing NACP from raw pointer
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct FromPtrError(FromBytesError);

/// System language codes (matches libnx SetLanguage).
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
pub enum SetLanguage {
    /// Japanese
    JA = 0,
    /// English (US)
    ENUS = 1,
    /// French
    FR = 2,
    /// German
    DE = 3,
    /// Italian
    IT = 4,
    /// Spanish
    ES = 5,
    /// Chinese (Simplified)
    ZHCN = 6,
    /// Korean
    KO = 7,
    /// Dutch
    NL = 8,
    /// Portuguese
    PT = 9,
    /// Russian
    RU = 10,
    /// Chinese (Traditional)
    ZHTW = 11,
    /// English (UK)
    ENGB = 12,
    /// French (Canada)
    FRCA = 13,
    /// Spanish (Latin America)
    ES419 = 14,
    /// Chinese (Simplified, alternative)
    ZHHANS = 15,
    /// Chinese (Traditional, alternative)
    ZHHANT = 16,
    /// Portuguese (Brazil)
    PTBR = 17,
}

/// Maps SetLanguage to NACP language entry index
const LANGUAGE_TABLE: [usize; 18] = [
    2,  // JA
    0,  // ENUS
    3,  // FR
    4,  // DE
    7,  // IT
    6,  // ES
    14, // ZHCN
    12, // KO
    8,  // NL
    10, // PT
    11, // RU
    13, // ZHTW
    1,  // ENGB
    9,  // FRCA
    5,  // ES419
    14, // ZHHANS (same as ZHCN)
    13, // ZHHANT (same as ZHTW)
    15, // PTBR
];

fn cstr_to_str(bytes: &[u8]) -> &str {
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..len]).unwrap_or("")
}
