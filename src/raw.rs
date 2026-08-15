//! The on-disk layouts themselves, as `#[repr(C)]` structures a byte slice can be cast to.
//!
//! Every structure here is byte-aligned and its size is asserted against the format it mirrors, so
//! it can be mapped onto a slice at any offset without copying and without a layout surprise. That
//! is also the constraint the module upholds: a field added, reordered, or widened here changes the
//! meaning of every image the crate reads, and the size assertion beside each structure is what
//! catches it.
//!
//! Nothing in this module validates. A cast succeeds on any slice of the right length, whatever the
//! bytes mean, and the magic numbers declared here are compared by [`crate::read`] rather than by
//! anything below it. Reach for these types to inspect a field directly; reach for [`crate::read`]
//! to be told when the image is not what it claims.
//!
//! Multi-byte fields are little-endian, spelled as `zerocopy` little-endian types rather than as
//! native integers, because the format is little-endian regardless of the host this crate runs on.

pub mod build_id;
#[cfg(feature = "cnmt")]
pub mod cnmt;
#[cfg(feature = "kip")]
pub mod kip;
#[cfg(feature = "mod0")]
pub mod mod0;
#[cfg(feature = "nacp")]
pub mod nacp;
#[cfg(feature = "nca")]
pub mod nca;
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
