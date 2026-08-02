//! Builders that assemble the Switch executable and asset formats.
//!
//! Every builder returns its finished image as a byte buffer rather than writing it,
//! so the caller chooses where an artifact lands and a failed build leaves nothing
//! behind on disk. [`NpdmBuilder::build`] is the one exception to their shape: it is
//! infallible, because it performs no validation of the metadata it was handed.

pub mod kip;
pub mod nacp;
pub mod npdm;
pub mod nro;
pub mod nso;
pub mod pfs0;
pub mod romfs;

pub use kip::Kip1Builder;
pub use nacp::NacpBuilder;
pub use npdm::NpdmBuilder;
pub use nro::NroBuilder;
pub use nso::NsoBuilder;
pub use pfs0::Pfs0Builder;
pub use romfs::RomFsBuilder;
