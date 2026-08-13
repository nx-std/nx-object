//! Builders that assemble the Switch executable and asset formats.
//!
//! Every builder returns its finished image as a byte buffer rather than writing it,
//! so the caller chooses where an artifact lands and a failed build leaves nothing
//! behind on disk.
//!
//! Each builder takes ownership at every step and hands it back, so a rejected input returns the
//! error instead of the builder and nothing partial survives. Validation happens as parts are
//! added, which is why some builds cannot fail at all: [`NpdmBuilder::build`] and
//! [`Pfs0Builder::build`] are infallible, the first because it validates none of the metadata it
//! is handed, the second because every name was already checked when its file was added.
//!
//! Insertion order does not reach an image. The builders whose formats hold several entries sort
//! them on build, so the same inputs always produce the same bytes and an artifact can be compared
//! against a rebuild.

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
