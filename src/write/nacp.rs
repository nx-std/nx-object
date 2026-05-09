//! NACP (Nintendo Application Control Property) builder.

use std::{string::String, vec::Vec};

use crate::{raw::nacp::NacpStruct, read::SetLanguage};

/// Builder for constructing NACP files.
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
    /// Create a new NACP builder with default values.
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

    /// Set the application name for all languages.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        for entry in &mut self.names {
            *entry = Some(name.clone());
        }
        self
    }

    /// Set the application name for a specific language.
    pub fn name_for_language(mut self, lang: SetLanguage, name: impl Into<String>) -> Self {
        if let Some(idx) = language_to_index(lang) {
            self.names[idx] = Some(name.into());
        }
        self
    }

    /// Set the author/publisher for all languages.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        let author = author.into();
        for entry in &mut self.authors {
            *entry = Some(author.clone());
        }
        self
    }

    /// Set the author/publisher for a specific language.
    pub fn author_for_language(mut self, lang: SetLanguage, author: impl Into<String>) -> Self {
        if let Some(idx) = language_to_index(lang) {
            self.authors[idx] = Some(author.into());
        }
        self
    }

    /// Set the display version string (shown in UI).
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.display_version = Some(version.into());
        self
    }

    /// Set the application ID.
    pub fn application_id(mut self, id: u64) -> Self {
        self.application_id = Some(id);
        self
    }

    /// Set the save data owner ID.
    pub fn save_data_owner_id(mut self, id: u64) -> Self {
        self.save_data_owner_id = Some(id);
        self
    }

    /// Set the user account save data size in bytes.
    pub fn user_account_save_data_size(mut self, size: u64) -> Self {
        self.user_account_save_data_size = Some(size);
        self
    }

    /// Set the user account save data journal size in bytes.
    pub fn user_account_save_data_journal_size(mut self, size: u64) -> Self {
        self.user_account_save_data_journal_size = Some(size);
        self
    }

    /// Build the NACP structure, returning the complete 0x4000-byte buffer.
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

        // Set default metadata fields to match C nacptool behavior
        // (vendor/switch-tools/src/nacptool.c lines 100-110)

        // startup_user_account = 1 (require user account selection)
        nacp.startup_user_account = 1;

        // supported_language_flag = 0xFFFF (all 16 languages)
        // C tool sets 0xbff (12 languages), but we fill all 16 slots so use 0xFFFF
        nacp.supported_language_flag = 0xFFFF.into();

        // data_loss_confirmation = 1 (require confirmation for data that could be lost)
        nacp.data_loss_confirmation = 1;

        // rating_age = all 0xFF (unrated for all regions)
        // C tool uses specific pattern, but linkle uses 0xFF which is safer default
        nacp.rating_age = [0xFF_u8 as i8; 0x20];

        // user_account_save_data_size = 0x3e00000 (65,011,712 bytes ≈ 62 MB)
        // Default save data size from C nacptool
        nacp.user_account_save_data_size = 0x3e00000_u64.into();

        // user_account_save_data_journal_size = 0x180000 (1,572,864 bytes ≈ 1.5 MB)
        // Default journal size from C nacptool
        nacp.user_account_save_data_journal_size = 0x180000_u64.into();

        // logo_type = 2, logo_handling = 1
        // Controls how the Nintendo logo is displayed
        nacp.logo_type = 2;
        nacp.logo_handling = 1;

        // Fill other fields with little-endian values
        if let Some(id) = self.application_id {
            // When application_id is set, populate all title-id related fields
            // as per linkle behavior (vendor/linkle/src/format/nacp.rs:251-277)
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
