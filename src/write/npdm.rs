//! NPDM (Nintendo Program Description Metadata) builder.
//!
//! This module provides types and builders for creating NPDM files, which contain
//! program metadata, access control information, and kernel capabilities for
//! Nintendo Switch applications and system modules.
//!
//! # Structure
//!
//! An NPDM file consists of three sections:
//! - META header: Basic program information (name, version, thread config)
//! - ACID: Access Control Info Descriptor (RSA-signed permissions, optional for homebrew)
//! - ACI0: Access Control Info (runtime permissions: filesystem, services, kernel caps)
//!
//! # Example
//!
//! ```no_run
//! # use nx_object::write::npdm::{NpdmBuilder, NpdmMetadata, KernelCapability};
//! let metadata = NpdmMetadata {
//!     name: "MyApp".to_string(),
//!     title_id: 0x0100000000010000,
//!     version: 0,
//!     main_thread_priority: 44,
//!     default_cpu_id: 0,
//!     main_thread_stack_size: 0x100000,
//!     flags: 0,
//! };
//!
//! let npdm_bytes = NpdmBuilder::new(metadata).build();
//! std::fs::write("output.npdm", npdm_bytes).unwrap();
//! ```

use std::collections::HashMap;

use zerocopy::FromZeros;

/// Memory region descriptor for MapRegion kernel capability.
///
/// Each descriptor specifies a region type and read-only flag.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapRegionDescriptor {
    /// Region type (6-bit value)
    pub region_type: u8,
    /// Read-only flag
    pub is_ro: bool,
}

/// Kernel capability descriptor.
///
/// Each capability type encodes specific kernel permissions using a bit-field
/// format defined by the Nintendo Switch kernel. The encoding includes a trailing
/// bit pattern that identifies the capability type.
///
/// See: <https://switchbrew.org/wiki/NPDM#Kernel_Capability_Descriptors>
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum KernelCapability {
    /// Kernel flags: thread priority range and CPU affinity mask.
    ///
    /// Trailer: `0b111` (3 trailing 1-bits)
    KernelFlags {
        /// Highest allowed thread priority (0-63, higher = lower priority)
        highest_thread_priority: u8,
        /// Lowest allowed thread priority (0-63)
        lowest_thread_priority: u8,
        /// Highest allowed CPU core ID (0-3 on Switch)
        highest_cpu_id: u8,
        /// Lowest allowed CPU core ID
        lowest_cpu_id: u8,
    },

    /// Allowed system calls.
    ///
    /// Maps syscall names to their numeric IDs. Encoded as up to 8 bitmask entries
    /// (192 syscalls total, 24 per entry).
    ///
    /// Trailer: `0b1111` (4 trailing 1-bits)
    Syscalls(HashMap<String, u64>),

    /// Memory region mapping with permissions.
    ///
    /// Encodes as two u32 descriptors (address + flags, size + flags).
    ///
    /// Trailer: `0b11_1111` (6 trailing 1-bits, 2 entries)
    Map {
        /// Physical address to map
        address: u64,
        /// Size of the mapping in bytes
        size: u64,
        /// Read-only flag
        is_ro: bool,
        /// I/O flag (device memory vs normal memory)
        is_io: bool,
    },

    /// Single page mapping permission.
    ///
    /// Trailer: `0b111_1111` (7 trailing 1-bits)
    MapPage(u64),

    /// Memory region mapping.
    ///
    /// Encodes up to 3 region descriptors in a single u32 descriptor.
    ///
    /// Trailer: `0b11_1111_1111` (10 trailing 1-bits)
    MapRegion(Vec<MapRegionDescriptor>),

    /// IRQ (interrupt) pair.
    ///
    /// Two IRQ numbers allowed for this process.
    ///
    /// Trailer: `0b111_1111_1111` (11 trailing 1-bits)
    IrqPair([u16; 2]),

    /// Application type (applet, application, system module).
    ///
    /// Trailer: `0b1_1111_1111_1111` (13 trailing 1-bits)
    ApplicationType(u16),

    /// Minimum kernel version required.
    ///
    /// Trailer: `0b11_1111_1111_1111` (14 trailing 1-bits)
    MinKernelVersion(u64),

    /// Handle table size (max number of kernel handles).
    ///
    /// Trailer: `0b111_1111_1111_1111` (15 trailing 1-bits)
    HandleTableSize(u16),

    /// Debug flags (allow/force debug).
    ///
    /// Trailer: `0b1111_1111_1111_1111` (16 trailing 1-bits)
    ///
    /// At most one of the three flags may be set (mutually exclusive).
    DebugFlags {
        /// Allow debugging this process (bit 17)
        allow_debug: bool,
        /// Force production debugging mode (bit 18)
        force_debug_prod: bool,
        /// Force debugging this process (bit 19)
        force_debug: bool,
    },
}

impl KernelCapability {
    /// Encode this capability into one or more u32 descriptors.
    ///
    /// Each capability type produces 1-6 u32 values with type-specific bit layouts
    /// and a trailing bit pattern for type identification.
    ///
    /// # Returns
    ///
    /// Vector of encoded u32 descriptors ready for serialization in ACI0/ACID KC sections.
    pub fn encode(&self) -> Vec<u32> {
        match self {
            KernelCapability::KernelFlags {
                highest_thread_priority,
                lowest_thread_priority,
                highest_cpu_id,
                lowest_cpu_id,
            } => {
                // Trailer: 0b111 (3 trailing 1-bits)
                // Bit layout: [31:24]=highest_cpu_id, [23:16]=lowest_cpu_id,
                //             [15:10]=highest_priority, [9:4]=lowest_priority, [3:0]=0b0111
                let mut val = 0b111u32;
                val |= (u32::from(*lowest_thread_priority) & 0x3F) << 4;
                val |= (u32::from(*highest_thread_priority) & 0x3F) << 10;
                val |= (u32::from(*lowest_cpu_id) & 0xFF) << 16;
                val |= (u32::from(*highest_cpu_id) & 0xFF) << 24;
                vec![val]
            }
            KernelCapability::Syscalls(syscalls) => {
                // Trailer: 0b1111 (4 trailing 1-bits)
                // Bit layout: [31:29]=index, [28:5]=bitmask (24 syscalls), [4:0]=0b01111
                // Maximum 8 entries (indices 0-7) to cover 192 syscalls total
                let mut masks = vec![0b1111u32; 8];
                let mut used = [false; 8];

                // Set index field for each mask
                for (idx, mask) in masks.iter_mut().enumerate() {
                    *mask |= ((idx as u32) & 0x7) << 29;
                }

                // Set bitmask bits for each syscall
                for syscall_val in syscalls.values() {
                    let syscall_id = *syscall_val;
                    let mask_idx = (syscall_id / 24) as usize;
                    let bit_offset = (syscall_id % 24) + 5;

                    if mask_idx < 8 && bit_offset < 32 {
                        masks[mask_idx] |= 1u32 << bit_offset;
                        used[mask_idx] = true;
                    }
                }

                // Remove unused masks (from high indices down)
                for idx in (0..8).rev() {
                    if !used[idx] {
                        masks.remove(idx);
                    }
                }

                masks
            }
            KernelCapability::Map {
                address,
                size,
                is_ro,
                is_io,
            } => {
                // Trailer: 0b11_1111 (6 trailing 1-bits, produces 2 descriptors)
                // C reference: npdmtool.c:721-729
                // Descriptor 1: [31]=is_ro, [30:7]=address[23:0], [6:0]=0b0011_1111
                // Descriptor 2: [31]=is_io_inverted, [27:24]=address[39:36], [23:7]=size[20:0], [6:0]=0b0011_1111

                // Descriptor 1: address[23:0] with is_ro flag
                let mut desc1 = (*address as u32) & 0x00FF_FFFF;
                if *is_ro {
                    desc1 |= 1u32 << 24;
                }
                let val1 = (desc1 << 7) | 0b11_1111u32;

                // Descriptor 2: size[20:0] + upper address bits [39:36] + is_io inverted
                let mut desc2 = (*size as u32) & 0x000F_FFFF; // size[20:0]
                desc2 |= (((*address >> 24) as u32) & 0xF) << 20; // address[39:36] at bits [23:20]

                // Invert is_io flag (XOR with 1) per C reference
                let is_io_inverted = !*is_io;
                if is_io_inverted {
                    desc2 |= 1u32 << 24;
                }
                let val2 = (desc2 << 7) | 0b11_1111u32;

                vec![val1, val2]
            }
            KernelCapability::MapPage(page) => {
                // Trailer: 0b111_1111 (7 trailing 1-bits)
                // Bit layout: [31:8]=page_address, [7:0]=0b0111_1111
                let mut val = 0b111_1111u32;
                val |= ((*page as u32) & 0x00FF_FFFF) << 8;
                vec![val]
            }
            KernelCapability::MapRegion(regions) => {
                // Trailer: 0b11_1111_1111 (10 trailing 1-bits)
                // C reference: npdmtool.c:774-778
                // Bit layout: [31:25]=region2, [24:18]=region1, [17:11]=region0, [10:0]=0b011_1111_1111
                // Each region: [6:0]=(region_type[5:0] | (is_ro << 6))
                let mut val = 0b11_1111_1111u32;

                for (i, region) in regions.iter().take(3).enumerate() {
                    let region_desc =
                        (u32::from(region.region_type) & 0x3F) | (u32::from(region.is_ro) << 6);
                    val |= region_desc << (11 + 7 * i);
                }

                vec![val]
            }
            KernelCapability::IrqPair(irq_pair) => {
                // Trailer: 0b111_1111_1111 (11 trailing 1-bits)
                // Bit layout: [31:22]=irq1, [21:12]=irq0, [11:0]=0b0111_1111_1111
                let mut val = 0b111_1111_1111u32;
                val |= (u32::from(irq_pair[0]) & 0x3FF) << 12;
                val |= (u32::from(irq_pair[1]) & 0x3FF) << 22;
                vec![val]
            }
            KernelCapability::ApplicationType(app_type) => {
                // Trailer: 0b1_1111_1111_1111 (13 trailing 1-bits)
                // Bit layout: [16:14]=app_type, [13:0]=0b01_1111_1111_1111
                let mut val = 0b1_1111_1111_1111u32;
                val |= (u32::from(*app_type) & 0x7) << 14;
                vec![val]
            }
            KernelCapability::MinKernelVersion(min_kernel) => {
                // Trailer: 0b11_1111_1111_1111 (14 trailing 1-bits)
                // Bit layout: [31:15]=kernel_version, [14:0]=0b011_1111_1111_1111
                let mut val = 0b11_1111_1111_1111u32;
                val |= ((*min_kernel as u32) & 0x0001_FFFF) << 15;
                vec![val]
            }
            KernelCapability::HandleTableSize(handle_table_size) => {
                // Trailer: 0b111_1111_1111_1111 (15 trailing 1-bits)
                // Bit layout: [25:16]=handle_table_size, [15:0]=0b0111_1111_1111_1111
                let mut val = 0b111_1111_1111_1111u32;
                val |= (u32::from(*handle_table_size) & 0x3FF) << 16;
                vec![val]
            }
            KernelCapability::DebugFlags {
                allow_debug,
                force_debug_prod,
                force_debug,
            } => {
                // Trailer: 0b1111_1111_1111_1111 (16 trailing 1-bits)
                // C reference: npdmtool.c:841
                // desc = (allow_debug & 1) | ((force_debug_prod & 1) << 1) | ((force_debug & 1) << 2)
                // Encoded as: (desc << 17) | 0xFFFF
                // Bit layout: [19]=force_debug, [18]=force_debug_prod, [17]=allow_debug, [16:0]=0b01111_1111_1111_1111
                let mut val = 0b1111_1111_1111_1111u32;
                if *allow_debug {
                    val |= 1u32 << 17;
                }
                if *force_debug_prod {
                    val |= 1u32 << 18;
                }
                if *force_debug {
                    val |= 1u32 << 19;
                }
                vec![val]
            }
        }
    }
}

/// Filesystem access control permissions.
/// Save data owner ID entry with accessibility flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDataOwnerId {
    /// Accessibility flags for this save data owner
    pub accessibility: u8,
    /// Save data owner ID
    pub id: u64,
}

/// Filesystem access control.
///
/// Specifies which filesystem content and save data the application can access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemAccess {
    /// Filesystem permission bitmask
    pub permissions: u64,
    /// Content owner IDs array (for ACI0/FAH section)
    pub content_owner_ids: Vec<u64>,
    /// Save data owner IDs array (for ACI0/FAH section)
    pub save_data_owner_ids: Vec<SaveDataOwnerId>,
    /// Content owner ID range (for ACID/FAC section)
    pub content_owner_id_min: u64,
    pub content_owner_id_max: u64,
    /// Save data owner ID range (for ACID/FAC section)
    pub save_data_owner_id_min: u64,
    pub save_data_owner_id_max: u64,
}

impl Default for FilesystemAccess {
    fn default() -> Self {
        Self {
            permissions: 0xFFFFFFFFFFFFFFFF, // All permissions for homebrew
            content_owner_ids: Vec::new(),
            save_data_owner_ids: Vec::new(),
            content_owner_id_min: 0,
            content_owner_id_max: 0xFFFFFFFFFFFFFFFF,
            save_data_owner_id_min: 0,
            save_data_owner_id_max: 0xFFFFFFFFFFFFFFFF,
        }
    }
}

/// Service access control list.
///
/// Lists IPC service names this application is allowed to access, along with
/// whether each service is hosted (provided) by this program or accessed as a client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceAccess {
    /// List of allowed service names with is_host flag.
    ///
    /// Each entry is `(service_name, is_host)` where:
    /// - `service_name`: Service name (max 8 bytes, e.g., "fsp-srv", "sm:", "hid")
    /// - `is_host`: `true` if this program provides the service (is a host),
    ///   `false` if this program accesses the service (is a client)
    pub services: Vec<(String, bool)>,
}

/// ACI0 (Access Control Info) section data.
///
/// Contains the actual runtime permissions for the application.
#[derive(Debug, Clone, PartialEq)]
pub struct AciData {
    /// Program ID (Title ID)
    pub program_id: u64,
    /// Filesystem access permissions
    pub filesystem_access: FilesystemAccess,
    /// Service access control list
    pub service_access: ServiceAccess,
    /// Kernel capabilities
    pub kernel_capabilities: Vec<KernelCapability>,
}

#[allow(clippy::derivable_impls)]
impl Default for AciData {
    fn default() -> Self {
        Self {
            program_id: 0,
            filesystem_access: FilesystemAccess::default(),
            service_access: ServiceAccess::default(),
            kernel_capabilities: Vec::new(),
        }
    }
}

/// ACID (Access Control Info Descriptor) section data.
///
/// Contains RSA-signed permissions that constrain the ACI0 section.
/// For homebrew, an empty ACID (all zeros except header) is typical.
#[derive(Debug, Clone, PartialEq)]
pub struct AcidData {
    /// Program ID range minimum
    pub program_id_min: u64,
    /// Program ID range maximum
    pub program_id_max: u64,
    /// Filesystem access permissions (must be superset of ACI0)
    pub filesystem_access: FilesystemAccess,
    /// Service access control list (must be superset of ACI0)
    pub service_access: ServiceAccess,
    /// Kernel capabilities (must be superset of ACI0)
    pub kernel_capabilities: Vec<KernelCapability>,
    /// ACID flags (production/unqualified approval, etc.)
    pub flags: u32,
}

impl Default for AcidData {
    fn default() -> Self {
        Self {
            program_id_min: 0,
            program_id_max: 0xFFFFFFFFFFFFFFFF,
            filesystem_access: FilesystemAccess::default(),
            service_access: ServiceAccess::default(),
            kernel_capabilities: Vec::new(),
            flags: 0,
        }
    }
}

/// NPDM core metadata.
///
/// Contains basic program information stored in the META header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpdmMetadata {
    /// Program name (UTF-8, max 16 bytes including null terminator)
    pub name: String,
    /// Program ID (Title ID) - used as default for ACI0 program_id if no explicit ACI is provided
    pub title_id: u64,
    /// Program version
    pub version: u32,
    /// Main thread priority (0-63, higher = lower priority)
    pub main_thread_priority: u8,
    /// Default CPU core for main thread (0-3)
    pub default_cpu_id: u8,
    /// Main thread stack size in bytes
    pub main_thread_stack_size: u32,
    /// MMU flags
    pub flags: u8,
}

/// NPDM builder.
///
/// Constructs a complete NPDM binary containing META header, ACID section,
/// and ACI0 section.
///
/// # Homebrew Mode (Default)
///
/// By default, the builder creates a homebrew-compatible NPDM with an empty ACID
/// section (0x240 bytes of zeros except for the ACID header). This is the standard
/// configuration for NRO and NSP homebrew packages.
///
/// # Example
///
/// ```no_run
/// # use nx_object::write::npdm::{NpdmBuilder, NpdmMetadata};
/// let metadata = NpdmMetadata {
///     name: "MyApp".to_string(),
///     title_id: 0x0100000000010000,
///     version: 0,
///     main_thread_priority: 44,
///     default_cpu_id: 0,
///     main_thread_stack_size: 0x100000,
///     flags: 0,
/// };
///
/// // Build with default ACI (program_id automatically defaults to metadata.title_id)
/// let npdm = NpdmBuilder::new(metadata).build();
/// ```
#[derive(Debug, Clone)]
pub struct NpdmBuilder {
    metadata: NpdmMetadata,
    aci: Option<AciData>,
    acid: Option<AcidData>,
}

impl NpdmBuilder {
    /// Create a new NPDM builder with core metadata.
    ///
    /// # Arguments
    ///
    /// * `metadata` - Core program metadata (name, title ID, thread config, etc.)
    ///
    /// # Returns
    ///
    /// A new builder instance with empty-ACID mode (default for homebrew).
    pub fn new(metadata: NpdmMetadata) -> Self {
        Self {
            metadata,
            aci: None,
            acid: None,
        }
    }

    /// Set the ACI0 section data (runtime permissions).
    ///
    /// # Arguments
    ///
    /// * `aci` - Access control info with filesystem, service, and kernel capabilities
    pub fn with_aci(mut self, aci: AciData) -> Self {
        self.aci = Some(aci);
        self
    }

    /// Set the ACID section data (signed permissions).
    ///
    /// # Arguments
    ///
    /// * `acid` - Access control info descriptor
    ///
    /// # Note
    ///
    /// Most homebrew applications should use empty-ACID mode (default) instead
    /// of providing an ACID section. Custom ACID sections require proper RSA
    /// signing which is not supported in homebrew mode.
    pub fn with_acid(mut self, acid: AcidData) -> Self {
        self.acid = Some(acid);
        self
    }

    /// Build the complete NPDM binary.
    ///
    /// # Returns
    ///
    /// A `Vec<u8>` containing the serialized NPDM file (META + ACID + ACI0 sections).
    ///
    /// # Structure
    ///
    /// - Offset 0x0: META header (0x80 bytes)
    /// - Offset 0x80: ACID section (0x240+ bytes, empty for homebrew)
    /// - After ACID: ACI0 section (0x40+ bytes + capability data)
    pub fn build(self) -> Vec<u8> {
        use zerocopy::IntoBytes;

        use crate::raw::npdm::{
            ACI0_MAGIC, ACID_MAGIC, Aci0Header, AcidHeader, META_MAGIC, NpdmHeader,
        };

        // Get or create default ACI data
        // If no explicit ACI is provided, default program_id to metadata.title_id
        let aci = self.aci.unwrap_or_else(|| AciData {
            program_id: self.metadata.title_id,
            ..Default::default()
        });
        let acid = self.acid.unwrap_or_default();

        // Build ACI0 subsections (FAC, SAC, KC)
        let (fac_data, sac_data, kc_data) = build_aci_sections(&aci);

        // Build ACID subsections (reuse ACI data for homebrew)
        let (acid_fac_data, acid_sac_data, acid_kc_data) = build_acid_sections(&acid);

        // Calculate ACI0 section layout
        let aci0_fac_offset = size_of::<Aci0Header>() as u32;
        let aci0_sac_offset = align_up_16(aci0_fac_offset + fac_data.len() as u32);
        let aci0_kc_offset = align_up_16(aci0_sac_offset + sac_data.len() as u32);
        let aci0_size = aci0_kc_offset + kc_data.len() as u32;

        // Calculate ACID section layout
        let acid_fac_offset = size_of::<AcidHeader>() as u32;
        let acid_sac_offset = align_up_16(acid_fac_offset + acid_fac_data.len() as u32);
        let acid_kc_offset = align_up_16(acid_sac_offset + acid_sac_data.len() as u32);
        let acid_size = acid_kc_offset + acid_kc_data.len() as u32;

        // Build META header
        let mut meta_header = NpdmHeader::new_zeroed();
        meta_header.magic = META_MAGIC.into();
        meta_header.flags = self.metadata.flags;
        meta_header.main_thread_priority = self.metadata.main_thread_priority;
        meta_header.main_thread_core_number = self.metadata.default_cpu_id;
        meta_header.version = self.metadata.version.into();
        meta_header.main_thread_stack_size = self.metadata.main_thread_stack_size.into();

        // Copy program name (UTF-8, null-terminated, max 16 bytes)
        let name_bytes = self.metadata.name.as_bytes();
        let copy_len = name_bytes.len().min(meta_header.name.len() - 1);
        meta_header.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);

        // Set ACID offset and size
        meta_header.acid_offset = (size_of::<NpdmHeader>() as u32).into();
        meta_header.acid_size = acid_size.into();

        // Set ACI0 offset and size (16-byte aligned after ACID)
        let aci0_offset = align_up_16(u32::from(meta_header.acid_offset) + acid_size);
        meta_header.aci_offset = aci0_offset.into();
        meta_header.aci_size = aci0_size.into();

        // Build ACID header (empty for homebrew)
        let mut acid_header = AcidHeader::new_zeroed();
        acid_header.magic = ACID_MAGIC.into();
        acid_header.size = (acid_size - 0x100).into(); // Excludes signature
        acid_header.flags = acid.flags.into();
        acid_header.program_id_min = acid.program_id_min.into();
        acid_header.program_id_max = acid.program_id_max.into();
        acid_header.fac_offset = acid_fac_offset.into();
        acid_header.fac_size = (acid_fac_data.len() as u32).into();
        acid_header.sac_offset = acid_sac_offset.into();
        acid_header.sac_size = (acid_sac_data.len() as u32).into();
        acid_header.kc_offset = acid_kc_offset.into();
        acid_header.kc_size = (acid_kc_data.len() as u32).into();

        // Build ACI0 header
        let mut aci0_header = Aci0Header::new_zeroed();
        aci0_header.magic = ACI0_MAGIC.into();
        aci0_header.program_id = aci.program_id.into();
        aci0_header.fac_offset = aci0_fac_offset.into();
        aci0_header.fac_size = (fac_data.len() as u32).into();
        aci0_header.sac_offset = aci0_sac_offset.into();
        aci0_header.sac_size = (sac_data.len() as u32).into();
        aci0_header.kc_offset = aci0_kc_offset.into();
        aci0_header.kc_size = (kc_data.len() as u32).into();

        // Assemble final NPDM binary
        let total_size = aci0_offset as usize + aci0_size as usize;
        let mut npdm = vec![0u8; total_size];

        // Write META header
        npdm[..size_of::<NpdmHeader>()].copy_from_slice(meta_header.as_bytes());

        // Write ACID section
        let acid_start = u32::from(meta_header.acid_offset) as usize;
        npdm[acid_start..acid_start + size_of::<AcidHeader>()]
            .copy_from_slice(acid_header.as_bytes());
        npdm[acid_start + acid_fac_offset as usize..][..acid_fac_data.len()]
            .copy_from_slice(&acid_fac_data);
        npdm[acid_start + acid_sac_offset as usize..][..acid_sac_data.len()]
            .copy_from_slice(&acid_sac_data);
        npdm[acid_start + acid_kc_offset as usize..][..acid_kc_data.len()]
            .copy_from_slice(&acid_kc_data);

        // Write ACI0 section
        let aci0_start = aci0_offset as usize;
        npdm[aci0_start..aci0_start + size_of::<Aci0Header>()]
            .copy_from_slice(aci0_header.as_bytes());
        npdm[aci0_start + aci0_fac_offset as usize..][..fac_data.len()].copy_from_slice(&fac_data);
        npdm[aci0_start + aci0_sac_offset as usize..][..sac_data.len()].copy_from_slice(&sac_data);
        npdm[aci0_start + aci0_kc_offset as usize..][..kc_data.len()].copy_from_slice(&kc_data);

        npdm
    }
}

/// Align value up to 16-byte boundary.
fn align_up_16(value: u32) -> u32 {
    (value + 0xF) & !0xF
}

/// Build ACI0 subsections (FAC, SAC, KC).
///
/// Returns (fac_data, sac_data, kc_data) as byte vectors.
fn build_aci_sections(aci: &AciData) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // Build FAH (Filesystem Access Header) for ACI0
    let fac_data = build_filesystem_access_header(&aci.filesystem_access);

    // Build SAC (Service Access Control)
    let sac_data = build_service_access_control(&aci.service_access);

    // Build KC (Kernel Capabilities)
    let kc_data = build_kernel_capabilities(&aci.kernel_capabilities);

    (fac_data, sac_data, kc_data)
}

/// Build ACID subsections (FAC, SAC, KC).
///
/// Returns (fac_data, sac_data, kc_data) as byte vectors.
fn build_acid_sections(acid: &AcidData) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // Build FAC (Filesystem Access Control) for ACID
    let fac_data = build_filesystem_access_control(&acid.filesystem_access);

    // Build SAC (Service Access Control)
    let sac_data = build_service_access_control(&acid.service_access);

    // Build KC (Kernel Capabilities)
    let kc_data = build_kernel_capabilities(&acid.kernel_capabilities);

    (fac_data, sac_data, kc_data)
}

/// Build Filesystem Access Header (FAH) for ACI0.
///
/// Format:
/// - u32 version (1)
/// - u64 permissions
/// - u32 content_owner_id_offset
/// - u32 content_owner_id_size
/// - u32 save_data_owner_id_offset
/// - u32 save_data_owner_id_size
fn build_filesystem_access_header(fs_access: &FilesystemAccess) -> Vec<u8> {
    use zerocopy::little_endian::{U32, U64};

    let mut data = Vec::new();

    // Version
    data.extend_from_slice(&U32::new(1).to_bytes());

    // Permissions
    data.extend_from_slice(&U64::new(fs_access.permissions).to_bytes());

    // Calculate COI section size
    let coi_size = if fs_access.content_owner_ids.is_empty() {
        0u32
    } else {
        (4 + fs_access.content_owner_ids.len() * 8) as u32
    };

    // Calculate SDOI section size (with alignment)
    let sdoi_size = if fs_access.save_data_owner_ids.is_empty() {
        0u32
    } else {
        let count = fs_access.save_data_owner_ids.len();
        let accessibility_size_aligned = (count + 3) & !3; // Round up to nearest multiple of 4
        (4 + accessibility_size_aligned + count * 8) as u32
    };

    // Content owner ID offset/size
    let coi_offset = 0x1Cu32; // sizeof(FAH)
    data.extend_from_slice(&U32::new(coi_offset).to_bytes());
    data.extend_from_slice(&U32::new(coi_size).to_bytes());

    // Save data owner ID offset/size
    let sdoi_offset = coi_offset + coi_size;
    data.extend_from_slice(&U32::new(sdoi_offset).to_bytes());
    data.extend_from_slice(&U32::new(sdoi_size).to_bytes());

    // Encode content_owner_ids section (if present)
    if !fs_access.content_owner_ids.is_empty() {
        // Count (u32)
        data.extend_from_slice(&U32::new(fs_access.content_owner_ids.len() as u32).to_bytes());
        // IDs array (u64[])
        for id in &fs_access.content_owner_ids {
            data.extend_from_slice(&U64::new(*id).to_bytes());
        }
    }

    // Encode save_data_owner_ids section (if present)
    if !fs_access.save_data_owner_ids.is_empty() {
        // Count (u32)
        data.extend_from_slice(&U32::new(fs_access.save_data_owner_ids.len() as u32).to_bytes());

        // Accessibility array (u8[], padded to multiple of 4)
        for entry in &fs_access.save_data_owner_ids {
            data.push(entry.accessibility);
        }
        // Add padding to align to 4-byte boundary
        let accessibility_len = fs_access.save_data_owner_ids.len();
        let padding = ((accessibility_len + 3) & !3) - accessibility_len;
        data.extend(std::iter::repeat_n(0, padding));

        // IDs array (u64[])
        for entry in &fs_access.save_data_owner_ids {
            data.extend_from_slice(&U64::new(entry.id).to_bytes());
        }
    }

    data
}

/// Build Filesystem Access Control (FAC) for ACID.
///
/// Format:
/// - u8 version (1)
/// - u8 content_owner_id_count
/// - u8 save_data_owner_id_count
/// - u8 padding
/// - u64 permissions
/// - u64 content_owner_id_min
/// - u64 content_owner_id_max
/// - u64 save_data_owner_id_min
/// - u64 save_data_owner_id_max
fn build_filesystem_access_control(fs_access: &FilesystemAccess) -> Vec<u8> {
    use zerocopy::little_endian::U64;

    let mut data = vec![
        1, // Version
        0, // content_owner_id_count
        0, // save_data_owner_id_count
        0, // padding
    ];

    // Permissions
    data.extend_from_slice(&U64::new(fs_access.permissions).to_bytes());

    // Content owner ID range
    data.extend_from_slice(&U64::new(fs_access.content_owner_id_min).to_bytes());
    data.extend_from_slice(&U64::new(fs_access.content_owner_id_max).to_bytes());

    // Save data owner ID range
    data.extend_from_slice(&U64::new(fs_access.save_data_owner_id_min).to_bytes());
    data.extend_from_slice(&U64::new(fs_access.save_data_owner_id_max).to_bytes());

    data
}

/// Build Service Access Control (SAC).
///
/// Each entry is:
/// - u8 control_byte (length-1, bit 7 = is_host)
/// - service_name bytes (1-8 chars)
fn build_service_access_control(sac: &ServiceAccess) -> Vec<u8> {
    let mut data = Vec::new();

    for (service_name, is_host) in &sac.services {
        let name_bytes = service_name.as_bytes();
        let len = name_bytes.len().min(8);

        if len == 0 {
            continue;
        }

        // Control byte: [7]=is_host (0 for client, 1 for host), [6:0]=length-1
        let mut control = (len - 1) as u8;
        if *is_host {
            control |= 0x80;
        }
        data.push(control);
        data.extend_from_slice(&name_bytes[..len]);
    }

    data
}

/// Build Kernel Capabilities (KC).
///
/// Encode all capabilities as u32 descriptors.
fn build_kernel_capabilities(capabilities: &[KernelCapability]) -> Vec<u8> {
    use zerocopy::little_endian::U32;

    let mut data = Vec::new();

    for cap in capabilities {
        let descriptors = cap.encode();
        for desc in descriptors {
            data.extend_from_slice(&U32::new(desc).to_bytes());
        }
    }

    data
}
