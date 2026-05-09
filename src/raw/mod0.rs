use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// MOD0 magic number: "MOD0" in ASCII (0x30444f4d).
pub const MOD0_MAGIC: u32 = 0x30444f4d;

/// MOD0 header structure embedded in NRO/NSO executables.
///
/// The MOD0 header provides metadata about the module's dynamic linking
/// information, BSS section, and exception handling tables. The header
/// location is specified in the NRO start header.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Mod0Header {
    /// Magic number (must be [`MOD0_MAGIC`])
    pub magic: U32,
    /// Offset to .dynamic section (relative to MOD0 base)
    pub dynamic_offset: I32,
    /// Offset to BSS start (relative to MOD0 base)
    pub bss_start_offset: I32,
    /// Offset to BSS end (relative to MOD0 base)
    pub bss_end_offset: I32,
    /// Offset to .eh_frame_hdr start (relative to MOD0 base)
    pub eh_frame_hdr_start: I32,
    /// Offset to .eh_frame_hdr end (relative to MOD0 base)
    pub eh_frame_hdr_end: I32,
    /// Offset to module object (relative to MOD0 base)
    pub module_object_offset: I32,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#MOD
const_assert_eq!(size_of::<Mod0Header>(), 0x1C);
const_assert_eq!(align_of::<Mod0Header>(), 0x1);
