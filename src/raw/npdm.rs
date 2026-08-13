//! The three NPDM section headers: META, ACID and ACI0.
//!
//! Each header is a fixed size, but the sections themselves are variable-length:
//! the META header records where ACID and ACI0 begin and how long they are, and
//! each of those in turn records the offsets of its own filesystem-access,
//! service-access and kernel-capability blocks.
//!
//! Every offset inside ACID or ACI0 is relative to that section's own start, not
//! to the image.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{U32, U64},
};

/// Magic identifying the NPDM META header: `META`, little-endian, at offset zero.
pub const META_MAGIC: u32 = 0x4154454d;

/// Magic identifying the ACID section: `ACID`, little-endian.
pub const ACID_MAGIC: u32 = 0x44494341;

/// Magic identifying the ACI0 section: `ACI0`, little-endian.
pub const ACI0_MAGIC: u32 = 0x30494341;

/// How the process is started, and where its two access-control sections are.
///
/// The root of an NPDM: it fixes the scheduling and memory the process is created with, and locates
/// [`AcidHeader`] and [`Aci0Header`], which carry the permissions themselves.
///
/// See <https://switchbrew.org/wiki/NPDM#Meta>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct NpdmHeader {
    /// Always [`META_MAGIC`]; anything else means this is not an NPDM.
    pub magic: U32,
    /// Which generation of signing key the ACID signature was produced with.
    pub signature_key_generation: U32,
    _reserved_08: U32,
    /// Process attributes: 64-bit or 32-bit, the address space size, and whether it may be debugged.
    pub flags: u8,
    _reserved_0d: u8,
    /// Priority the main thread starts at, from `0` to `63`, lower being more favourable.
    pub main_thread_priority: u8,
    /// Core the main thread is scheduled on by default.
    pub main_thread_core_number: u8,
    _reserved_10: U32,
    /// Extra memory granted to the process for system resources, in bytes.
    pub system_resource_size: U32,
    /// Program version, as the system compares it when deciding whether an update applies.
    pub version: U32,
    /// Stack reserved for the main thread, in bytes.
    pub main_thread_stack_size: U32,
    /// Program name, UTF-8 and NUL-padded to 16 bytes.
    pub name: [u8; 16],
    /// Product code, NUL-padded to 16 bytes, empty for homebrew.
    pub product_code: [u8; 16],
    _reserved_40: [u8; 48],
    /// Offset of the ACI0 section, from the start of the image.
    pub aci_offset: U32,
    /// Length of the ACI0 section in bytes.
    pub aci_size: U32,
    /// Offset of the ACID section, from the start of the image.
    pub acid_offset: U32,
    /// Length of the ACID section in bytes.
    pub acid_size: U32,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#Meta
const_assert_eq!(size_of::<NpdmHeader>(), 0x80);
const_assert_eq!(align_of::<NpdmHeader>(), 0x1);

/// The permissions a signing authority granted, and the program IDs they were granted to.
///
/// ACID is the signed half of the pair: it states the maximum a program may be given, and the
/// signature is what makes that binding. ACI0 then requests some subset of it, and the loader
/// rejects a program asking for more than its ACID allows.
///
/// See <https://switchbrew.org/wiki/NPDM#ACID>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct AcidHeader {
    /// RSA-2048 signature over everything in this section after the signature itself.
    pub signature: [u8; 0x100],
    /// RSA-2048 modulus the signature is verified against.
    pub public_key: [u8; 0x100],
    /// Always [`ACID_MAGIC`]; anything else means the offset did not point at an ACID section.
    pub magic: U32,
    /// Length of the signed region in bytes, which excludes the `0x100`-byte signature above it.
    pub size: U32,
    /// ACID format revision.
    pub version: u8,
    _reserved_209: [u8; 3],
    /// Whether the descriptor is for production, and whether it was approved unqualified.
    pub flags: U32,
    /// Lowest program ID this descriptor may be applied to.
    pub program_id_min: U64,
    /// Highest program ID this descriptor may be applied to.
    pub program_id_max: U64,
    /// Offset of the filesystem-access block, from the start of this section.
    pub fac_offset: U32,
    /// Length of the filesystem-access block in bytes.
    pub fac_size: U32,
    /// Offset of the service-access block, from the start of this section.
    pub sac_offset: U32,
    /// Length of the service-access block in bytes.
    pub sac_size: U32,
    /// Offset of the kernel-capability block, from the start of this section.
    pub kc_offset: U32,
    /// Length of the kernel-capability block in bytes.
    pub kc_size: U32,
    _reserved_238: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#ACID
const_assert_eq!(size_of::<AcidHeader>(), 0x240);
const_assert_eq!(align_of::<AcidHeader>(), 0x1);

/// The permissions this program actually requests, and the program ID it runs as.
///
/// ACI0 is the unsigned half of the pair: it carries the same three blocks as [`AcidHeader`] and
/// is checked against it at load, so nothing here grants anything the descriptor did not already
/// allow. Its blocks are laid out identically, which is what lets the loader compare them directly.
///
/// See <https://switchbrew.org/wiki/NPDM#ACI0>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Aci0Header {
    /// Always [`ACI0_MAGIC`]; anything else means the offset did not point at an ACI0 section.
    pub magic: U32,
    _reserved_04: [u8; 12],
    /// Title ID the process runs as, which must fall inside the ACID's permitted range.
    pub program_id: U64,
    _reserved_18: U64,
    /// Offset of the filesystem-access block, from the start of this section.
    pub fac_offset: U32,
    /// Length of the filesystem-access block in bytes.
    pub fac_size: U32,
    /// Offset of the service-access block, from the start of this section.
    pub sac_offset: U32,
    /// Length of the service-access block in bytes.
    pub sac_size: U32,
    /// Offset of the kernel-capability block, from the start of this section.
    pub kc_offset: U32,
    /// Length of the kernel-capability block in bytes.
    pub kc_size: U32,
    _reserved_38: U64,
}

// Verify struct size - https://switchbrew.org/wiki/NPDM#ACI0
const_assert_eq!(size_of::<Aci0Header>(), 0x40);
const_assert_eq!(align_of::<Aci0Header>(), 0x1);
