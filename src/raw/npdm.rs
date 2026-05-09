use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// META magic number: "META" in ASCII (0x4154454d).
pub const META_MAGIC: u32 = 0x4154454d;

/// ACID magic number: "ACID" in ASCII (0x44494341).
pub const ACID_MAGIC: u32 = 0x44494341;

/// ACI0 magic number: "ACI0" in ASCII (0x30494341).
pub const ACI0_MAGIC: u32 = 0x30494341;

/// NPDM META header - root header containing program metadata.
///
/// The META header is the top-level structure in an NPDM file, containing
/// basic program information and offsets to the ACID and ACI0 sections.
///
/// See: <https://switchbrew.org/wiki/NPDM>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NpdmHeader {
    /// Magic number (must be [`META_MAGIC`])
    pub magic: U32,
    /// Signature key generation
    pub signature_key_generation: U32,
    /// Reserved
    _reserved_08: U32,
    /// Flags
    pub flags: u8,
    /// Reserved
    _reserved_0d: u8,
    /// Main thread priority (0-63, higher = lower priority)
    pub main_thread_priority: u8,
    /// Default CPU core for main thread
    pub main_thread_core_number: u8,
    /// Reserved
    _reserved_10: U32,
    /// System resource size
    pub system_resource_size: U32,
    /// Program version
    pub version: U32,
    /// Main thread stack size in bytes
    pub main_thread_stack_size: U32,
    /// Program name (UTF-8, null-terminated)
    pub name: [u8; 16],
    /// Product code
    pub product_code: [u8; 16],
    /// Reserved
    _reserved_40: [u8; 48],
    /// Offset to ACI0 section (relative to NPDM start)
    pub aci_offset: U32,
    /// Size of ACI0 section in bytes
    pub aci_size: U32,
    /// Offset to ACID section (relative to NPDM start)
    pub acid_offset: U32,
    /// Size of ACID section in bytes
    pub acid_size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#Meta
const_assert_eq!(size_of::<NpdmHeader>(), 0x80);
const_assert_eq!(align_of::<NpdmHeader>(), 0x1);

/// ACID (Access Control Info Descriptor) header - signed access control.
///
/// The ACID section contains RSA-signed permissions and capabilities.
/// It specifies allowed program ID ranges and contains offsets to
/// filesystem access control (FAC), service access control (SAC),
/// and kernel capabilities (KC) data.
///
/// See: <https://switchbrew.org/wiki/NPDM#ACID>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct AcidHeader {
    /// RSA-2048 signature over ACID header + data
    pub signature: [u8; 0x100],
    /// RSA-2048 public key for signature verification
    pub public_key: [u8; 0x100],
    /// Magic number (must be [`ACID_MAGIC`])
    pub magic: U32,
    /// Size of ACID section excluding the 0x100-byte RSA signature
    pub size: U32,
    /// ACID version
    pub version: u8,
    /// Reserved
    _reserved_209: [u8; 3],
    /// Flags (production/unqualified approval, etc.)
    pub flags: U32,
    /// Minimum allowed program ID
    pub program_id_min: U64,
    /// Maximum allowed program ID
    pub program_id_max: U64,
    /// Offset to FAC data (relative to ACID start)
    pub fac_offset: U32,
    /// Size of FAC data
    pub fac_size: U32,
    /// Offset to SAC data (relative to ACID start)
    pub sac_offset: U32,
    /// Size of SAC data
    pub sac_size: U32,
    /// Offset to KC data (relative to ACID start)
    pub kc_offset: U32,
    /// Size of KC data
    pub kc_size: U32,
    /// Reserved
    _reserved_238: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#ACID
const_assert_eq!(size_of::<AcidHeader>(), 0x240);
const_assert_eq!(align_of::<AcidHeader>(), 0x1);

/// ACI0 (Access Control Info) header - actual program permissions.
///
/// The ACI0 section contains the actual access control information
/// that will be used at runtime. Similar structure to ACID but without
/// signature/public key and program ID range.
///
/// See: <https://switchbrew.org/wiki/NPDM#ACI0>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Aci0Header {
    /// Magic number (must be [`ACI0_MAGIC`])
    pub magic: U32,
    /// Reserved
    _reserved_04: [u8; 12],
    /// Program ID (Title ID)
    pub program_id: U64,
    /// Reserved
    _reserved_18: U64,
    /// Offset to FAC data (relative to ACI0 start)
    pub fac_offset: U32,
    /// Size of FAC data
    pub fac_size: U32,
    /// Offset to SAC data (relative to ACI0 start)
    pub sac_offset: U32,
    /// Size of SAC data
    pub sac_size: U32,
    /// Offset to KC data (relative to ACI0 start)
    pub kc_offset: U32,
    /// Size of KC data
    pub kc_size: U32,
    /// Reserved
    _reserved_38: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#ACI0
const_assert_eq!(size_of::<Aci0Header>(), 0x40);
const_assert_eq!(align_of::<Aci0Header>(), 0x1);
