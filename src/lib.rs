//! Zero-copy parsing and generation of Nintendo Switch file formats.
//!
//! Turns a byte buffer into a validated view of an NRO, NSO, NACP, NPDM, or RomFS image, and builds
//! each of those formats back out of its parts. Nothing is copied to read an image and nothing is
//! written to disk to produce one, so the same definitions serve a host-side packer and code
//! running on the console.
//!
//! # The three layers
//!
//! Each layer is usable on its own, and each is a different answer to how much the caller wants
//! done for them.
//!
//! | Layer         | What it gives you                                            | Available            |
//! |---------------|--------------------------------------------------------------|----------------------|
//! | [`mod@raw`]   | The on-disk layouts as `#[repr(C)]` structures, unvalidated   | always               |
//! | [`mod@read`]  | Views that validate once, then hand out parts without checks  | always               |
//! | [`mod@write`] | Builders returning a finished image as a byte buffer          | `filesystem-support` |
//! | [`mod@elf`]   | Segments extracted from a linked ELF, ready for a builder     | `elf-parsing`        |
//!
//! Reach for [`mod@raw`] to inspect a field directly, and for [`mod@read`] to be told when the image
//! is not what it claims. A [`mod@read`] type that exists is one whose bounds have been proven,
//! which is why its accessors return plain slices rather than `Result`.
//!
//! # `no_std`
//!
//! The crate is `no_std` unless `filesystem-support` is enabled, and that feature is what brings in
//! `std` along with the [`mod@write`] layer. Parsing therefore needs no allocator; building an image
//! does, since a builder has to put the assembled bytes somewhere.
//!
//! # Formats
//!
//! | Format | What it is                                    | [`mod@raw`] | [`mod@read`] | [`mod@write`] |
//! |--------|-----------------------------------------------|:-----------:|:------------:|:-------------:|
//! | NRO    | The executable the homebrew menu launches     |      ✓      |      ✓       |       ✓       |
//! | NSO    | The executable format system modules use      |      ✓      |      ✓       |       ✓       |
//! | KIP    | An initial process the kernel starts directly |      ✓      |              |       ✓       |
//! | NACP   | How a title is presented and what it may do   |      ✓      |      ✓       |       ✓       |
//! | NPDM   | The permissions a program is granted          |      ✓      |      ✓       |       ✓       |
//! | RomFS  | The read-only filesystem a title ships        |      ✓      |      ✓       |       ✓       |
//! | PFS0   | The flat archive an NSP is built from         |      ✓      |              |       ✓       |
//! | MOD0   | The runtime header embedded in an executable  |      ✓      |      ✓       |               |
//!
//! # What this crate does not do
//!
//! It does not sign, encrypt, or verify anything. An NPDM's ACID signature is stored and reproduced
//! but never checked, and an NSO's segment hashes are computed on write yet left to the caller on
//! read. It also does not decompress: an NSO segment comes back as stored, because a reader that
//! borrows its buffer has nowhere to put the expanded bytes.
//!
//! # References
//!
//! Every format is documented on the switchbrew wiki, and each module links the page for the
//! structure it mirrors.
//!
//! - [NRO](https://switchbrew.org/wiki/NRO)
//! - [NSO](https://switchbrew.org/wiki/NSO)
//! - [KIP](https://switchbrew.org/wiki/KIP)
//! - [NACP](https://switchbrew.org/wiki/NACP)
//! - [NPDM](https://switchbrew.org/wiki/NPDM)
//! - [RomFS](https://switchbrew.org/wiki/RomFS)
//! - [PFS0](https://switchbrew.org/wiki/PFS0)

#![cfg_attr(not(feature = "filesystem-support"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "filesystem-support")]
mod blz;
#[cfg(feature = "elf-parsing")]
pub mod elf;
pub mod raw;
pub mod read;
#[cfg(feature = "filesystem-support")]
pub mod write;
