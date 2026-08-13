//! Validated reader over a NACP control structure.
//!
//! A NACP carries no magic, so [`Nacp::try_from_bytes`] can only check that the
//! buffer is large enough — any 0x4000 bytes will parse. A caller that has not
//! established the bytes are a NACP by other means learns nothing from success
//! here.
//!
//! Title and publisher live in a 16-entry language table that is sparse: an entry
//! is present only for the languages the title ships, so lookups return `Option`
//! and [`Nacp::language_entry_for`] falls back to the first populated entry.

use zerocopy::FromBytes;

use crate::raw::nacp::{NacpLanguageEntry, NacpStruct};

/// A borrowed view of a NACP, with the language table's sparseness handled.
///
/// Holding one is no evidence the bytes are a NACP: the format has no magic, so anything of the
/// right length parses. What this type adds over the raw structure is the language lookup, which
/// has to distinguish an absent entry from an empty one.
pub struct Nacp<'a> {
    raw: &'a NacpStruct,
}

impl<'a> Nacp<'a> {
    /// Borrow `bytes` as a NACP, checking only that there are enough of them.
    ///
    /// # Errors
    ///
    /// Returns an error only if the buffer is shorter than a NACP. There is no
    /// magic to check, so any buffer of sufficient length parses and success says
    /// nothing about whether the bytes are really a NACP.
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

    /// The underlying structure, for the fields this view does not interpret.
    pub fn raw(&self) -> &NacpStruct {
        self.raw
    }

    /// The title and publisher at `index` in the language table, or `None` when that slot is unused.
    ///
    /// A slot is unused when its name begins with a NUL, which is how a zeroed table entry is told
    /// apart from a populated one. An `index` past the end of the table is `None` rather than a
    /// panic, so a language value from an image cannot bring the reader down.
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

    /// The version as shown to the user, up to the first NUL, empty when unset.
    pub fn display_version(&self) -> &str {
        cstr_to_str(&self.raw.display_version)
    }

    /// The title and publisher for `lang`, falling back to the first populated language.
    ///
    /// The fallback is what the console does: a title that ships no entry for the console's
    /// language is still displayed, under whichever name it does carry. `None` means the table is
    /// entirely empty, which is a NACP naming the title in no language at all.
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

    /// Read a NACP already mapped in memory, for a program inspecting its own metadata.
    ///
    /// Unlike the executable formats, the length is known: a NACP is always `0x4000` bytes, so
    /// exactly that many are borrowed.
    ///
    /// # Safety
    ///
    /// - `ptr` points at `0x4000` readable bytes in one allocation.
    /// - That mapping stays live and unwritten for `'a`.
    ///
    /// # Errors
    ///
    /// Cannot fail in practice: the slice is built at exactly the required length, so the length
    /// check in [`Nacp::try_from_bytes`] always passes. The `Result` is kept so the two
    /// constructors report failure the same way.
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

/// System language codes.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
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
