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

#[cfg(feature = "kip")]
pub mod kip;
#[cfg(feature = "nacp")]
pub mod nacp;
#[cfg(feature = "npdm")]
pub mod npdm;
#[cfg(feature = "nro")]
pub mod nro;
#[cfg(feature = "nso")]
pub mod nso;
#[cfg(feature = "pfs0")]
pub mod pfs0;
#[cfg(feature = "romfs")]
pub mod romfs;

#[cfg(feature = "kip")]
pub use kip::Kip1Builder;
#[cfg(feature = "nacp")]
pub use nacp::NacpBuilder;
#[cfg(feature = "npdm")]
pub use npdm::NpdmBuilder;
#[cfg(feature = "nro")]
pub use nro::NroBuilder;
#[cfg(feature = "nso")]
pub use nso::NsoBuilder;
#[cfg(feature = "pfs0")]
pub use pfs0::Pfs0Builder;
#[cfg(feature = "romfs")]
pub use romfs::RomFsBuilder;
