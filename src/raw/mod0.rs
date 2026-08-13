//! The MOD0 header embedded in NRO and NSO executables.
//!
//! Every offset in the header is signed and relative to the MOD0 header's own
//! address, not to the start of the image — the header sits inside the text
//! segment, so the sections it points at may precede it.

use static_assertions::const_assert_eq;
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    little_endian::{I32, U32},
};

/// Magic identifying a MOD0 header: `MOD0`, little-endian.
pub const MOD0_MAGIC: u32 = 0x30444f4d;

/// What the runtime needs to relocate itself: the dynamic section, the BSS bounds, and the
/// unwind tables.
///
/// The header is embedded in the `text` segment of an NRO or NSO rather than stored beside it, and
/// [`NroStart::mod_offset`] is what locates it. Every offset it holds is signed and measured from
/// the header's own address, so a section placed before the header is reached through a negative
/// value.
///
/// [`NroStart::mod_offset`]: crate::raw::nro::NroStart::mod_offset
///
/// See <https://switchbrew.org/wiki/NRO#MOD>.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Mod0Header {
    /// Always [`MOD0_MAGIC`]; anything else means the offset did not point at a MOD0 header.
    pub magic: U32,
    /// Offset of the `.dynamic` section, from this header's address.
    pub dynamic_offset: I32,
    /// Offset of the first byte of BSS, from this header's address.
    pub bss_start_offset: I32,
    /// Offset one past the last byte of BSS, from this header's address.
    ///
    /// The runtime zeroes the half-open range between the two BSS offsets on startup.
    pub bss_end_offset: I32,
    /// Offset of the first byte of `.eh_frame_hdr`, from this header's address.
    pub eh_frame_hdr_start: I32,
    /// Offset one past the last byte of `.eh_frame_hdr`, from this header's address.
    pub eh_frame_hdr_end: I32,
    /// Offset of the runtime's module object, from this header's address.
    pub module_object_offset: I32,
}

// Verify struct size - https://switchbrew.org/wiki/NRO#MOD
const_assert_eq!(size_of::<Mod0Header>(), 0x1C);
const_assert_eq!(align_of::<Mod0Header>(), 0x1);
