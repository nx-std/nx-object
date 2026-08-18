//! Readers that validate an image once, then hand out its parts without re-checking.
//!
//! Every type here borrows the buffer it was given rather than copying it, and every check happens
//! at construction. That is the invariant the module upholds and the reason its accessors return
//! plain slices instead of `Result`: a reader that exists is one whose bounds have been proven, so
//! a crafted image fails at the door rather than panicking three calls later.
//!
//! Two things deliberately fall outside it. Bounds that cannot be checked without reading the whole
//! image are checked on use instead, which is why walking a RomFS entry is fallible while reading an
//! NRO segment is not. And a format with no magic, NACP and RomFS among them, cannot be recognized
//! at all: those readers check lengths and layout, so success means the buffer is shaped like the
//! format, never that it is one.
//!
//! No accessor here decrypts or verifies a hash. An NSO segment comes back as stored and checking
//! it against the header's digest is the caller's, because a borrowing reader has nowhere to put
//! the expanded bytes. The same line puts NCA decryption outside: [`nca::Nca`] takes a buffer the
//! caller has already decrypted, and an image still in ciphertext fails its magic check rather than
//! parsing into nonsense.
//!
//! Decompression sits just on the other side of that line, and is offered as an explicit step
//! rather than folded into an accessor: [`kip::Kip1Segment::decompress`] allocates and can fail,
//! which is exactly why it is a call a reader makes on purpose and not a slice it is handed.

#[cfg(feature = "cnmt")]
pub mod cnmt;
#[cfg(all(feature = "alloc", feature = "kip"))]
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
