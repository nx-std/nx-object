//! # nx-object
//! Zero-copy parsing and generation of Nintendo Switch executable formats.
//!
//! This crate provides three layers:
//! - `raw`: Low-level binary structure definitions using `zerocopy`
//! - `read`: High-level parsing wrappers with error handling
//! - `write`: Builder patterns for constructing formats (requires `std` feature)
//!
//! Supported formats:
//! - **NRO** (Nintendo Relocatable Object) - Homebrew executable format
//! - **NSO** (Nintendo Software Object) - Official software module format
//! - **NACP** (Nintendo Application Control Property) - Application metadata
//! - **NPDM** (Nintendo Program Description Metadata) - Program permissions
//! - **RomFS** - Read-only filesystem used in application bundles
//! - **MOD0** - Module header embedded in executables
//!
//! # References
//! - [switchbrew NRO](https://switchbrew.org/wiki/NRO)
//! - [switchbrew NSO](https://switchbrew.org/wiki/NSO)
//! - [switchbrew NACP](https://switchbrew.org/wiki/NACP)
//! - [switchbrew NPDM](https://switchbrew.org/wiki/NPDM)
//! - [switchbrew RomFS](https://switchbrew.org/wiki/RomFS)

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
mod blz;
#[cfg(feature = "elf")]
pub mod elf;
pub mod raw;
pub mod read;
#[cfg(feature = "std")]
pub mod write;
