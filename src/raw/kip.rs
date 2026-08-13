//! KIP1 header and segment descriptors.
//!
//! A KIP1 is the initial-process format: a 0x100-byte header followed by six
//! segments whose descriptors carry both the compressed and decompressed sizes,
//! because the loader decompresses each segment in place.
//!
//! Both structures are byte-aligned and size-asserted, so either may be mapped
//! onto a slice at any offset.

use static_assertions::const_assert_eq;
use zerocopy::little_endian::{U32, U64};

/// Magic identifying a KIP1 header: `KIP1`, little-endian, at offset zero.
pub const KIP1_MAGIC: u32 = 0x3150494b;

/// Where one segment lands in memory, how large it is stored and expanded, and what it carries.
///
/// Both lengths are present because the kernel decompresses BLZ segments in place: it needs the
/// stored length to find the compressed bytes and the final length to know how much room to leave.
/// When the segment's compression bit in [`Kip1Header::flags`] is clear, the two are equal.
///
/// See <https://switchbrew.org/wiki/KIP1#Segment_Header>.
#[derive(Debug, Clone, Copy, zerocopy::FromZeros, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
pub struct Kip1Segment {
    /// Address the segment is loaded at, relative to the process image base.
    pub dst_addr: U32,
    /// Length of the segment once decompressed, in bytes.
    pub decomp_size: U32,
    /// Length of the segment as stored in the file, in bytes.
    pub comp_size: U32,
    /// Meaning depends on the segment: for `rodata` it is the main thread's stack size, in bytes.
    ///
    /// The field is positional rather than typed, so a value read here is only meaningful once the
    /// segment it belongs to is known.
    pub attributes: U32,
}

// Verify struct size - https://switchbrew.org/wiki/KIP1#Segment_Header
const_assert_eq!(size_of::<Kip1Segment>(), 0x10);
const_assert_eq!(align_of::<Kip1Segment>(), 0x1);

/// Everything the kernel needs to start an initial process: its identity, its segments, and the
/// capabilities it is granted.
///
/// Occupies the first `0x100` bytes of the file. A KIP is launched by the kernel before any
/// filesystem exists, so the header carries what a loader would otherwise read from an NPDM.
///
/// See <https://switchbrew.org/wiki/KIP1#KIP1>.
#[derive(Debug, Clone, Copy, zerocopy::FromZeros, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
pub struct Kip1Header {
    /// Always [`KIP1_MAGIC`]; anything else means this is not a KIP1.
    pub magic: U32,
    /// Process name, NUL-padded to 12 bytes. A 12-character name has no terminator.
    pub name: [u8; 12],
    /// Title ID the process runs under.
    pub title_id: U64,
    /// Process category, which decides how the kernel schedules and privileges the process.
    pub process_category: U32,
    /// Priority the main thread starts at, lower being more favourable.
    pub main_thread_priority: u8,
    /// Core the main thread is scheduled on by default.
    pub default_cpu_id: u8,
    _reserved: u8,
    /// Compression bits for the first three segments, and the process's address-space shape.
    ///
    /// Bits `0`, `1`, and `2` mark `text`, `rodata`, and `data` as BLZ-compressed; bit `3` selects
    /// a 64-bit process, bit `4` a 32-bit address space, and bit `5` the system pool partition.
    /// Clearing a compression bit without rewriting its segment leaves the kernel expanding bytes
    /// that are already expanded.
    pub flags: u8,
    /// The `text`, `rodata`, `data`, and `bss` segments, followed by two unused descriptors.
    pub segments: [Kip1Segment; 6],
    /// Kernel capability descriptors, `0x20` packed `u32` values.
    ///
    /// These are the syscalls, memory regions, and interrupts the process is permitted; an
    /// unused slot is filled with `0xFFFFFFFF`.
    pub capabilities: [u8; 0x80],
}

// Verify struct size - https://switchbrew.org/wiki/KIP1#KIP1
const_assert_eq!(size_of::<Kip1Header>(), 0x100);
const_assert_eq!(align_of::<Kip1Header>(), 0x1);
