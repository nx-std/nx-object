//! Builder patterns for constructing Nintendo Switch executable formats.
//!
//! This module provides builders for creating NRO, NSO, NACP, and RomFS files.
//! All builders follow a consistent pattern:
//!
//! 1. Create a new builder with `Builder::new()`
//! 2. Configure it with chainable setter methods
//! 3. Call `.build()` to generate the final byte buffer
//!
//! # Example
//!
//! ```no_run
//! use nx_object::write::NacpBuilder;
//!
//! let nacp = NacpBuilder::new()
//!     .name("My Homebrew")
//!     .author("Developer")
//!     .version("1.0.0")
//!     .build()
//!     .expect("failed to build NACP");
//! ```

#[cfg(feature = "blz")]
pub mod kip;
pub mod nacp;
pub mod npdm;
pub mod nro;
pub mod nso;
pub mod pfs0;
pub mod romfs;

#[cfg(feature = "blz")]
pub use kip::Kip1Builder;
pub use nacp::NacpBuilder;
pub use npdm::NpdmBuilder;
pub use nro::NroBuilder;
pub use nso::NsoBuilder;
pub use pfs0::Pfs0Builder;
pub use romfs::RomFsBuilder;
