//! Builder for the NACP control structure a title is presented by.
//!
//! The structure has no length and no magic: it is always `0x4000` bytes, so building one means
//! filling fields at fixed offsets in a zeroed buffer rather than appending. A field left unset is
//! therefore written as zero, which the console reads as absent.
//!
//! Titles are held per language until the structure is built, because a name set for all languages
//! and a name set for one have to compose, and the last writer for a given language wins.

use std::{string::String, vec::Vec};

use crate::{raw::nacp::NacpStruct, read::SetLanguage};

/// Fills in the fixed `0x4000`-byte control structure a title is described by.
///
/// Every field starts zeroed, so an unset field is written as absent rather than defaulted.
pub struct NacpBuilder {
    names: [Option<String>; 16],
    authors: [Option<String>; 16],
    display_version: Option<String>,
    application_id: Option<u64>,
    save_data_owner_id: Option<u64>,
    user_account_save_data_size: Option<u64>,
    user_account_save_data_journal_size: Option<u64>,
}

impl NacpBuilder {
    /// Start a control structure with every field zeroed.
    pub fn new() -> Self {
        Self {
            names: Default::default(),
            authors: Default::default(),
            display_version: None,
            application_id: None,
            save_data_owner_id: None,
            user_account_save_data_size: None,
            user_account_save_data_journal_size: None,
        }
    }

    /// Give every language the same title, for a release not localized per language.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        for entry in &mut self.names {
            *entry = Some(name.clone());
        }
        self
    }

    /// Give one language its own title, overriding what was set for all of them.
    pub fn name_for_language(mut self, lang: SetLanguage, name: impl Into<String>) -> Self {
        if let Some(idx) = language_to_index(lang) {
            self.names[idx] = Some(name.into());
        }
        self
    }

    /// Give every language the same publisher.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        let author = author.into();
        for entry in &mut self.authors {
            *entry = Some(author.clone());
        }
        self
    }

    /// Give one language its own publisher, overriding what was set for all of them.
    pub fn author_for_language(mut self, lang: SetLanguage, author: impl Into<String>) -> Self {
        if let Some(idx) = language_to_index(lang) {
            self.authors[idx] = Some(author.into());
        }
        self
    }

    /// Set the version shown to the user, such as `1.0.0`, which the system never parses.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.display_version = Some(version.into());
        self
    }

    /// Set the title ID this NACP describes.
    pub fn application_id(mut self, id: u64) -> Self {
        self.application_id = Some(id);
        self
    }

    /// Set the title ID owning the save data, which lets a title read a predecessor's saves.
    pub fn save_data_owner_id(mut self, id: u64) -> Self {
        self.save_data_owner_id = Some(id);
        self
    }

    /// Reserve per-account save data, in bytes; `0` gives the title no account saves.
    pub fn user_account_save_data_size(mut self, size: u64) -> Self {
        self.user_account_save_data_size = Some(size);
        self
    }

    /// Reserve the journal bounding one save transaction's writes, in bytes.
    pub fn user_account_save_data_journal_size(mut self, size: u64) -> Self {
        self.user_account_save_data_journal_size = Some(size);
        self
    }

    /// Return the finished control structure, zero-filled wherever nothing was set.
    ///
    /// # Errors
    ///
    /// Returns an error if a name, author, or version exceeds the fixed width its
    /// field reserves. The fields are not truncated to fit, because a silently
    /// shortened title is indistinguishable from an intended one.
    pub fn build(self) -> Result<Vec<u8>, BuildError> {
        // Create zeroed buffer
        let mut buf = vec![0u8; 0x4000];

        // Parse as NacpStruct for field access
        let nacp_ref = zerocopy::Ref::<&mut [u8], NacpStruct>::from_bytes(&mut buf[..])
            .map_err(|_| BuildError::InternalBufferSizeError)?;
        let nacp = zerocopy::Ref::into_mut(nacp_ref);

        // Fill language entries
        for (i, (name_opt, author_opt)) in self.names.iter().zip(self.authors.iter()).enumerate() {
            let entry = &mut nacp.lang[i];

            if let Some(name) = name_opt {
                let name_bytes = name.as_bytes();
                if name_bytes.len() >= 0x200 {
                    return Err(BuildError::NameTooLong {
                        language_index: i,
                        len: name_bytes.len(),
                    });
                }
                entry.name[..name_bytes.len()].copy_from_slice(name_bytes);
            }

            if let Some(author) = author_opt {
                let author_bytes = author.as_bytes();
                if author_bytes.len() >= 0x100 {
                    return Err(BuildError::AuthorTooLong {
                        language_index: i,
                        len: author_bytes.len(),
                    });
                }
                entry.author[..author_bytes.len()].copy_from_slice(author_bytes);
            }
        }

        // Fill display version (offset 0x3060)
        if let Some(version) = self.display_version {
            let version_bytes = version.as_bytes();
            if version_bytes.len() >= 0x10 {
                return Err(BuildError::VersionTooLong {
                    len: version_bytes.len(),
                });
            }
            nacp.display_version[..version_bytes.len()].copy_from_slice(version_bytes);
        }

        // Set default metadata fields for a standard homebrew application

        // startup_user_account = 1 (require user account selection)
        nacp.startup_user_account = 1;

        // supported_language_flag = 0xFFFF: all 16 language slots are populated
        nacp.supported_language_flag = 0xFFFF.into();

        // data_loss_confirmation = 1 (require confirmation for data that could be lost)
        nacp.data_loss_confirmation = 1;

        // rating_age = all 0xFF (unrated for all regions) — a safe default
        nacp.rating_age = [0xFF_u8 as i8; 0x20];

        // user_account_save_data_size = 0x3e00000 (65,011,712 bytes ≈ 62 MB)
        nacp.user_account_save_data_size = 0x3e00000_u64.into();

        // user_account_save_data_journal_size = 0x180000 (1,572,864 bytes ≈ 1.5 MB)
        nacp.user_account_save_data_journal_size = 0x180000_u64.into();

        // logo_type = 2, logo_handling = 1
        // Controls how the Nintendo logo is displayed
        nacp.logo_type = 2;
        nacp.logo_handling = 1;

        // Fill other fields with little-endian values
        if let Some(id) = self.application_id {
            // When application_id is set, populate all title-id related fields
            nacp.presence_group_id = id.into();
            nacp.add_on_content_base_id = (id + 0x1000).into(); // DLC base offset
            nacp.save_data_owner_id = id.into();
            nacp.local_communication_id[0] = id.into();
            nacp.local_communication_id[1] = id.into();
            nacp.pseudo_device_id_seed = id.into();
        }
        if let Some(id) = self.save_data_owner_id {
            nacp.save_data_owner_id = id.into();
        }
        if let Some(size) = self.user_account_save_data_size {
            nacp.user_account_save_data_size = size.into();
        }
        if let Some(size) = self.user_account_save_data_journal_size {
            nacp.user_account_save_data_journal_size = size.into();
        }

        Ok(buf)
    }
}

impl Default for NacpBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned by [`NacpBuilder::build`].
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Application name exceeds maximum length (0x200 bytes).
    #[error("name too long for language {language_index}: {len} bytes (max 0x200)")]
    NameTooLong {
        /// Language entry index (0-15)
        language_index: usize,
        /// Actual length in bytes
        len: usize,
    },
    /// Author/publisher name exceeds maximum length (0x100 bytes).
    #[error("author too long for language {language_index}: {len} bytes (max 0x100)")]
    AuthorTooLong {
        /// Language entry index (0-15)
        language_index: usize,
        /// Actual length in bytes
        len: usize,
    },
    /// Display version string exceeds maximum length (0x10 bytes).
    #[error("version too long: {len} bytes (max 0x10)")]
    VersionTooLong {
        /// Actual length in bytes
        len: usize,
    },
    /// Internal error: buffer size mismatch (should never happen).
    #[error("internal buffer size error")]
    InternalBufferSizeError,
}

/// Map SetLanguage to NACP language entry index.
fn language_to_index(lang: SetLanguage) -> Option<usize> {
    Some(match lang {
        SetLanguage::ENUS => 0,
        SetLanguage::ENGB => 1,
        SetLanguage::JA => 2,
        SetLanguage::FR => 3,
        SetLanguage::DE => 4,
        SetLanguage::ES419 => 5,
        SetLanguage::ES => 6,
        SetLanguage::IT => 7,
        SetLanguage::NL => 8,
        SetLanguage::FRCA => 9,
        SetLanguage::PT => 10,
        SetLanguage::RU => 11,
        SetLanguage::KO => 12,
        SetLanguage::ZHTW | SetLanguage::ZHHANT => 13,
        SetLanguage::ZHCN | SetLanguage::ZHHANS => 14,
        SetLanguage::PTBR => 15,
    })
}
