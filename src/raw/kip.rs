use static_assertions::const_assert_eq;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, little_endian::*};

/// KIP1 magic number: "KIP1" in ASCII (0x3150494b).
pub const KIP1_MAGIC: u32 = 0x3150494b;

/// KIP1 segment descriptor (text, rodata, data, or bss).
///
/// Describes location, size, and attributes of a segment within the KIP file.
/// KIP segments can be BLZ-compressed.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Kip1Segment {
    /// Destination address in memory
    pub dst_addr: U32,
    /// Decompressed size in bytes
    pub decomp_size: U32,
    /// Compressed size in bytes (if compression enabled, otherwise equals decomp_size)
    pub comp_size: U32,
    /// Segment attributes (e.g., main thread stack size for rodata segment)
    pub attributes: U32,
}

// Verify struct size - https://switchbrew.org/wiki/KIP#Segment_Header
const_assert_eq!(size_of::<Kip1Segment>(), 0x10);
const_assert_eq!(align_of::<Kip1Segment>(), 0x1);

/// KIP1 header (0x100 bytes).
///
/// Contains metadata, segment descriptors, and kernel capabilities for a kernel
/// initial process.
///
/// See: <https://switchbrew.org/wiki/KIP>
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct Kip1Header {
    /// Magic number (must be [`KIP1_MAGIC`])
    pub magic: U32,
    /// Process name (12 bytes, null-terminated)
    pub name: [u8; 12],
    /// Title ID
    pub title_id: U64,
    /// Process category
    pub process_category: U32,
    /// Main thread priority
    pub main_thread_priority: u8,
    /// Default CPU core ID
    pub default_cpu_id: u8,
    /// Reserved (must be 0)
    _reserved: u8,
    /// Flags byte:
    /// - Bit 0-2: Compression flags (text, rodata, data)
    /// - Bit 3: Is64Bit
    /// - Bit 4: IsAddrSpace32Bit
    /// - Bit 5: UseSystemPoolPartition
    pub flags: u8,
    /// Array of segment descriptors: [text, rodata, data, bss, reserved, reserved]
    pub segments: [Kip1Segment; 6],
    /// Kernel capability descriptors (0x80 bytes = 32 u32 values)
    pub capabilities: [u8; 0x80],
}

// Verify struct size - https://switchbrew.org/wiki/KIP#KIP_Header
const_assert_eq!(size_of::<Kip1Header>(), 0x100);
const_assert_eq!(align_of::<Kip1Header>(), 0x1);
