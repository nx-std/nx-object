use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// Language-specific title and publisher information.
///
/// NACP contains 16 language entries for different regions.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpLanguageEntry {
    /// Application name (UTF-8, null-terminated)
    pub name: [u8; 0x200],
    /// Publisher/developer name (UTF-8, null-terminated)
    pub author: [u8; 0x100],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#Title
const_assert_eq!(size_of::<NacpLanguageEntry>(), 0x300);
const_assert_eq!(align_of::<NacpLanguageEntry>(), 0x1);

/// Local wireless neighbor detection group configuration.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpNeighborDetectionGroupConfig {
    /// Group identifier
    pub group_id: U64,
    /// Encryption key for the group
    pub key: [u8; 0x10],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#NeighborDetectionGroupConfiguration
const_assert_eq!(size_of::<NacpNeighborDetectionGroupConfig>(), 0x18);
const_assert_eq!(align_of::<NacpNeighborDetectionGroupConfig>(), 0x1);

/// Local wireless neighbor detection client configuration.
///
/// Configures groups for sending and receiving local wireless communication.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpNeighborDetectionClientConfig {
    /// Configuration for the group this client sends to
    pub send_group_config: NacpNeighborDetectionGroupConfig,
    /// Array of up to 16 groups this client can receive from
    pub receivable_group_configs: [NacpNeighborDetectionGroupConfig; 0x10],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#NeighborDetectionClientConfiguration
const_assert_eq!(size_of::<NacpNeighborDetectionClientConfig>(), 0x198);
const_assert_eq!(align_of::<NacpNeighborDetectionClientConfig>(), 0x1);

/// Just-In-Time compilation configuration.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpJitConfiguration {
    /// JIT flags
    pub flags: U64,
    /// Maximum JIT memory size in bytes
    pub memory_size: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NACP#JitConfiguration
const_assert_eq!(size_of::<NacpJitConfiguration>(), 0x10);
const_assert_eq!(align_of::<NacpJitConfiguration>(), 0x1);

/// Nintendo Application Control Property (NACP) - complete 0x4000-byte structure.
///
/// Contains application metadata including titles, ratings, save data configuration,
/// and system capabilities. This structure is embedded in NRO files and NSP packages.
///
/// See: <https://switchbrew.org/wiki/NACP>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpStruct {
    /// Array of 16 language entries (title/author per language)
    pub lang: [NacpLanguageEntry; 16],
    /// ISBN code (null-terminated string)
    pub isbn: [u8; 0x25],
    /// Startup user account mode
    pub startup_user_account: u8,
    /// User account switch lock mode
    pub user_account_switch_lock: u8,
    /// Add-on content registration type
    pub add_on_content_registration_type: u8,
    /// Application attribute flags
    pub attribute_flag: U32,
    /// Supported language flags (bitmask)
    pub supported_language_flag: U32,
    /// Parental control flags
    pub parental_control_flag: U32,
    /// Screenshot permission
    pub screenshot: u8,
    /// Video capture permission
    pub video_capture: u8,
    /// Data loss confirmation requirement
    pub data_loss_confirmation: u8,
    /// Play log policy
    pub play_log_policy: u8,
    /// Presence group ID for online features
    pub presence_group_id: U64,
    /// Age rating values per region (0x20 regions)
    pub rating_age: [i8; 0x20],
    /// Display version string (null-terminated UTF-8)
    pub display_version: [u8; 0x10],
    /// Base ID for add-on content
    pub add_on_content_base_id: U64,
    /// Save data owner ID
    pub save_data_owner_id: U64,
    /// User account save data size in bytes
    pub user_account_save_data_size: U64,
    /// User account save data journal size in bytes
    pub user_account_save_data_journal_size: U64,
    /// Device save data size in bytes
    pub device_save_data_size: U64,
    /// Device save data journal size in bytes
    pub device_save_data_journal_size: U64,
    /// BCAT delivery cache storage size in bytes
    pub bcat_delivery_cache_storage_size: U64,
    /// Application error code category string (as u64)
    pub application_error_code_category: U64,
    /// Array of up to 8 local communication IDs
    pub local_communication_id: [U64; 0x8],
    /// Logo type
    pub logo_type: u8,
    /// Logo handling mode
    pub logo_handling: u8,
    /// Runtime add-on content install permission
    pub runtime_add_on_content_install: u8,
    /// Runtime parameter delivery permission
    pub runtime_parameter_delivery: u8,
    /// Reserved (0x30f4)
    _reserved_x30f4: [u8; 0x2],
    /// Crash report mode
    pub crash_report: u8,
    /// HDCP mode
    pub hdcp: u8,
    /// Pseudo device ID seed
    pub pseudo_device_id_seed: U64,
    /// BCAT passphrase (null-terminated string)
    pub bcat_passphrase: [u8; 0x41],
    /// Startup user account option flags
    pub startup_user_account_option: u8,
    /// Reserved for user account save data operations
    _reserved_user_account_save_data_op: [u8; 0x6],
    /// Maximum user account save data size in bytes
    pub user_account_save_data_size_max: U64,
    /// Maximum user account save data journal size in bytes
    pub user_account_save_data_journal_size_max: U64,
    /// Maximum device save data size in bytes
    pub device_save_data_size_max: U64,
    /// Maximum device save data journal size in bytes
    pub device_save_data_journal_size_max: U64,
    /// Temporary storage size in bytes
    pub temporary_storage_size: U64,
    /// Cache storage size in bytes
    pub cache_storage_size: U64,
    /// Cache storage journal size in bytes
    pub cache_storage_journal_size: U64,
    /// Maximum combined cache storage data and journal size
    pub cache_storage_data_and_journal_size_max: U64,
    /// Maximum cache storage index
    pub cache_storage_index_max: U16,
    /// Reserved (0x318a)
    _reserved_x318a: [u8; 0x6],
    /// Array of up to 16 queryable application IDs for play log
    pub play_log_queryable_application_id: [U64; 0x10],
    /// Play log query capability
    pub play_log_query_capability: u8,
    /// Repair flags
    pub repair_flag: u8,
    /// Program index within multi-program applications
    pub program_index: u8,
    /// Requirement for network service license on launch
    pub required_network_service_license_on_launch: u8,
    /// Reserved (0x3214)
    _reserved_x3214: U32,
    /// Local wireless neighbor detection configuration
    pub neighbor_detection_client_config: NacpNeighborDetectionClientConfig,
    /// JIT compilation configuration
    pub jit_configuration: NacpJitConfiguration,
    /// Reserved padding to reach 0x4000 total size
    _reserved_x33c0: [u8; 0xc40],
}

// Verify struct size - https://switchbrew.org/wiki/NACP
const_assert_eq!(size_of::<NacpStruct>(), 0x4000);
const_assert_eq!(align_of::<NacpStruct>(), 0x1);
