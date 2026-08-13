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
//! | Layer   | What it gives you                                            | Available     |
//! |---------|--------------------------------------------------------------|---------------|
//! | `raw`   | The on-disk layouts as `#[repr(C)]` structures, unvalidated   | always        |
//! | `read`  | Views that validate once, then hand out parts without checks  | always        |
//! | `write` | Builders returning a finished image as a byte buffer          | `alloc`       |
//! | `elf`   | Segments extracted from a linked ELF, ready for a builder     | `elf-parsing` |
//!
//! Reach for `raw` to inspect a field directly, and for `read` to be told when the image is not what
//! it claims. A `read` type that exists is one whose bounds have been proven, which is why its
//! accessors return plain slices rather than `Result`.
//!
//! The layers are named here rather than linked, because a build that leaves `write` or `elf` out
//! would have nothing for the link to resolve to and the table describes the crate whole.
//!
//! # `no_std`
//!
//! The crate is `no_std` unless `std` is enabled, and it needs an allocator only where the work
//! genuinely does. Three tiers, each a feature:
//!
//! - **Bare `no_std`** gives `raw` and `read`. Parsing borrows the buffer it is handed, so it
//!   allocates nothing at all.
//! - **`alloc`** adds `write`. A builder has to put the assembled bytes somewhere, and a heap is the
//!   whole of what it needs.
//! - **`std`** adds the `from_directory` builders and the path-carrying errors they report, which
//!   are the only things here that touch a filesystem.
//!
//! A format is a feature too, and `all-formats` turns on every one. The default is `std` plus that,
//! so a consumer who names nothing gets the whole crate.
//!
//! # Formats
//!
//! | Format | What it is                                    | `raw` | `read` | `write` |
//! |--------|-----------------------------------------------|:-----:|:------:|:-------:|
//! | NRO    | The executable the homebrew menu launches     |   ✓   |   ✓    |    ✓    |
//! | NSO    | The executable format system modules use      |   ✓   |   ✓    |    ✓    |
//! | KIP    | An initial process the kernel starts directly |   ✓   |        |    ✓    |
//! | NACP   | How a title is presented and what it may do   |   ✓   |   ✓    |    ✓    |
//! | NPDM   | The permissions a program is granted          |   ✓   |   ✓    |    ✓    |
//! | RomFS  | The read-only filesystem a title ships        |   ✓   |   ✓    |    ✓    |
//! | PFS0   | The flat archive an NSP is built from         |   ✓   |        |    ✓    |
//! | MOD0   | The runtime header embedded in an executable  |   ✓   |   ✓    |         |
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
//! structure it mirrors. RomFS is the exception: switchbrew has no page for it, and the layout is
//! the one the console inherits unchanged from the 3DS.
//!
//! - [NRO](https://switchbrew.org/wiki/NRO)
//! - [NSO](https://switchbrew.org/wiki/NSO)
//! - [KIP1](https://switchbrew.org/wiki/KIP1)
//! - [NACP](https://switchbrew.org/wiki/NACP)
//! - [NPDM](https://switchbrew.org/wiki/NPDM)
//! - [MOD](https://switchbrew.org/wiki/MOD)
//! - [PFS0](https://switchbrew.org/wiki/NCA#PFS0)
//! - [RomFS](https://www.3dbrew.org/wiki/RomFS)

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Compressing a KIP1's segments is the only thing this is for, so it follows that builder rather
// than the allocator it happens to need.
#[cfg(all(feature = "alloc", feature = "kip"))]
mod blz;
#[cfg(feature = "elf-parsing")]
pub mod elf;
pub mod raw;
pub mod read;
#[cfg(feature = "alloc")]
pub mod write;
