//! The NACP control structure and its nested configuration blocks.
//!
//! A NACP is a fixed 0x4000 bytes with no length prefix or terminator: every
//! field sits at a constant offset and unused space is zero-filled, so the
//! structure is mapped whole rather than parsed incrementally.
//!
//! Text fields are fixed-width, null-padded byte arrays rather than strings; a
//! value that fills its array has no terminator.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{U16, U32, U64},
};

/// The title and publisher shown for one system language.
///
/// A NACP carries one entry per language, at a fixed index, so the console picks a title by
/// indexing rather than by searching. An entry left zeroed means the title offers no name in that
/// language and the console falls back to another.
///
/// See <https://switchbrew.org/wiki/NACP#ApplicationTitle>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpLanguageEntry {
    /// Title as displayed on the home screen, UTF-8 and NUL-padded to `0x200` bytes.
    pub name: [u8; 0x200],
    /// Publisher as displayed beneath the title, UTF-8 and NUL-padded to `0x100` bytes.
    pub author: [u8; 0x100],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#ApplicationTitle
const_assert_eq!(size_of::<NacpLanguageEntry>(), 0x300);
const_assert_eq!(align_of::<NacpLanguageEntry>(), 0x1);

/// One local-wireless group a title takes part in, and the key that admits it.
///
/// See <https://switchbrew.org/wiki/NACP#NeighborDetectionGroupConfiguration>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpNeighborDetectionGroupConfig {
    /// Identifier consoles use to recognize the group, or `0` when no group is configured.
    pub group_id: U64,
    /// Key admitting a console to the group.
    pub key: [u8; 0x10],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#NeighborDetectionGroupConfiguration
const_assert_eq!(size_of::<NacpNeighborDetectionGroupConfig>(), 0x18);
const_assert_eq!(align_of::<NacpNeighborDetectionGroupConfig>(), 0x1);

/// Which local-wireless groups a title broadcasts to, and which it listens to.
///
/// The two directions are configured separately, so a title can be discoverable by consoles it
/// cannot itself discover.
///
/// See <https://switchbrew.org/wiki/NACP#NeighborDetectionClientConfiguration>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpNeighborDetectionClientConfig {
    /// The group this title broadcasts to.
    pub send_group_config: NacpNeighborDetectionGroupConfig,
    /// The groups this title accepts broadcasts from, zeroed entries being unused slots.
    pub receivable_group_configs: [NacpNeighborDetectionGroupConfig; 0x10],
}

// Verify struct size - https://switchbrew.org/wiki/NACP#NeighborDetectionClientConfiguration
const_assert_eq!(size_of::<NacpNeighborDetectionClientConfig>(), 0x198);
const_assert_eq!(align_of::<NacpNeighborDetectionClientConfig>(), 0x1);

/// Whether the title may generate code at runtime, and how much memory it may do so in.
///
/// See <https://switchbrew.org/wiki/NACP#JitConfiguration>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpJitConfiguration {
    /// Whether the title is permitted to use the JIT service; `0` denies it.
    pub flags: U64,
    /// Upper bound on memory the title may map as executable at runtime, in bytes.
    pub memory_size: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NACP#JitConfiguration
const_assert_eq!(size_of::<NacpJitConfiguration>(), 0x10);
const_assert_eq!(align_of::<NacpJitConfiguration>(), 0x1);

/// Everything the system knows about a title without launching it: how it is presented, what it is
/// permitted to do, and how much storage it is owed.
///
/// A fixed `0x4000` bytes with no length prefix and no terminator. Every field sits at a constant
/// offset and unused space is zeroed, so the structure is mapped whole rather than parsed, and a
/// field this crate does not name is still present and still zeroed.
///
/// Most single-byte fields select one of a small set of documented modes rather than being
/// booleans; the accepted values for each are listed on the wiki.
///
/// See <https://switchbrew.org/wiki/NACP>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NacpStruct {
    /// Title and publisher per system language, indexed by language rather than searched.
    pub lang: [NacpLanguageEntry; 16],
    /// ISBN of the work the title is based on, NUL-padded, empty when there is none.
    pub isbn: [u8; 0x25],
    /// Whether launching the title requires a user account to be selected first.
    pub startup_user_account: u8,
    /// Whether the user may switch accounts while the title runs.
    pub user_account_switch_lock: u8,
    /// How add-on content is registered for the title.
    pub add_on_content_registration_type: u8,
    /// Marks the title as a demo, a retail display unit, or neither.
    pub attribute_flag: U32,
    /// Which entries of `lang` are populated, as a bitmask indexed the same way.
    pub supported_language_flag: U32,
    /// Whether parental controls apply to the title, and to which of its features.
    pub parental_control_flag: U32,
    /// Whether the user may capture screenshots while the title runs.
    pub screenshot: u8,
    /// Whether the user may capture video while the title runs, and whether it is automatic.
    pub video_capture: u8,
    /// Whether the system warns about unsaved progress before closing the title.
    pub data_loss_confirmation: u8,
    /// Whether play history is recorded for the title, and who may query it.
    pub play_log_policy: u8,
    /// Groups titles that report presence together, so friends see one activity for all of them.
    pub presence_group_id: U64,
    /// Minimum age per rating organization, indexed by organization; `-1` where unrated.
    pub rating_age: [i8; 0x20],
    /// Version as shown to the user, such as `1.0.0`, NUL-padded and never parsed by the system.
    pub display_version: [u8; 0x10],
    /// Title ID add-on content for this title is registered against.
    pub add_on_content_base_id: U64,
    /// Title ID owning the save data, which lets a title read a predecessor's saves.
    pub save_data_owner_id: U64,
    /// Save data reserved per user account, in bytes; `0` means the title has no account saves.
    pub user_account_save_data_size: U64,
    /// Journal reserved for per-account save data, in bytes, bounding one transaction's writes.
    pub user_account_save_data_journal_size: U64,
    /// Save data reserved for the console rather than an account, in bytes.
    pub device_save_data_size: U64,
    /// Journal reserved for device save data, in bytes.
    pub device_save_data_journal_size: U64,
    /// Storage reserved for content delivered in the background, in bytes.
    pub bcat_delivery_cache_storage_size: U64,
    /// Prefix the system prepends to error codes the title reports, as packed ASCII.
    pub application_error_code_category: U64,
    /// Identifiers this title communicates under over local wireless; unused slots are `0`.
    pub local_communication_id: [U64; 0x8],
    /// Which logo the system shows while the title starts.
    pub logo_type: u8,
    /// Whether the system draws the startup logo or leaves it to the title.
    pub logo_handling: u8,
    /// Whether add-on content may be installed while the title is running.
    pub runtime_add_on_content_install: u8,
    /// Whether launch parameters may be delivered to the title after it has started.
    pub runtime_parameter_delivery: u8,
    _reserved_x30f4: [u8; 0x2],
    /// Whether a crash of this title may be reported to Nintendo.
    pub crash_report: u8,
    /// Whether the title's video output requires HDCP.
    pub hdcp: u8,
    /// Seed mixing into the per-title device identifier, so the same console looks different to
    /// different titles.
    pub pseudo_device_id_seed: U64,
    /// Passphrase authenticating the title against the background download service, NUL-padded.
    pub bcat_passphrase: [u8; 0x41],
    /// Refines the account requirement in `startup_user_account`.
    pub startup_user_account_option: u8,
    _reserved_user_account_save_data_op: [u8; 0x6],
    /// Ceiling the per-account save data may be extended to, in bytes.
    pub user_account_save_data_size_max: U64,
    /// Ceiling the per-account save journal may be extended to, in bytes.
    pub user_account_save_data_journal_size_max: U64,
    /// Ceiling the device save data may be extended to, in bytes.
    pub device_save_data_size_max: U64,
    /// Ceiling the device save journal may be extended to, in bytes.
    pub device_save_data_journal_size_max: U64,
    /// Scratch storage the title may use while running, in bytes, discarded when it exits.
    pub temporary_storage_size: U64,
    /// Storage the title may cache downloaded content in, in bytes, which the system may reclaim.
    pub cache_storage_size: U64,
    /// Journal reserved for cache storage, in bytes.
    pub cache_storage_journal_size: U64,
    /// Ceiling on cache storage and its journal combined, in bytes.
    pub cache_storage_data_and_journal_size_max: U64,
    /// Highest cache storage index the title may open, so a title may hold several caches.
    pub cache_storage_index_max: U16,
    _reserved_x318a: [u8; 0x6],
    /// Titles whose play history this one may query; unused slots are `0`.
    pub play_log_queryable_application_id: [U64; 0x10],
    /// Whether the title may query play history at all, and whose.
    pub play_log_query_capability: u8,
    /// Marks the title as needing attention from the system's repair flow.
    pub repair_flag: u8,
    /// Position of this program within a multi-program title, `0` for a single-program one.
    pub program_index: u8,
    /// Whether a network service license must be present for the title to launch.
    pub required_network_service_license_on_launch: u8,
    _reserved_x3214: U32,
    /// Local-wireless groups the title broadcasts to and listens to.
    pub neighbor_detection_client_config: NacpNeighborDetectionClientConfig,
    /// Whether the title may generate code at runtime, and in how much memory.
    pub jit_configuration: NacpJitConfiguration,
    _reserved_x33c0: [u8; 0xc40],
}

// Verify struct size - https://switchbrew.org/wiki/NACP
const_assert_eq!(size_of::<NacpStruct>(), 0x4000);
const_assert_eq!(align_of::<NacpStruct>(), 0x1);
