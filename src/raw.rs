//! Raw binary structure definitions for Nintendo Switch executable formats.
//!
//! This module contains zero-copy struct definitions using the `zerocopy` crate.
//! All structures are defined with `#[repr(C)]` and match the official format specifications.
//!
//! Use these types when you need direct access to binary fields without parsing overhead.
//! For higher-level parsing with error handling, see the `read` module.

pub mod build_id;
pub mod kip;
pub mod mod0;
pub mod nacp;
pub mod npdm;
pub mod nro;
pub mod nso;
pub mod pfs0;
pub mod romfs;
