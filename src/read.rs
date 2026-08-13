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
//! Nothing here decompresses or verifies a hash. An NSO segment comes back as stored, and checking
//! it against the header's digest is the caller's, because a borrowing reader has nowhere to put
//! the expanded bytes.

mod mod0;
mod nacp;
mod npdm;
mod nro;
mod nso;
mod romfs;

pub use self::{
    mod0::{FromBytesError as Mod0FromBytesError, FromPtrError as Mod0FromPtrError, Mod0},
    nacp::{
        FromBytesError as NacpFromBytesError, FromPtrError as NacpFromPtrError, Nacp, SetLanguage,
    },
    npdm::{FromBytesError as NpdmFromBytesError, Npdm},
    nro::{FromBytesError as NroFromBytesError, FromPtrError as NroFromPtrError, Nro},
    nso::{FromBytesError as NsoFromBytesError, FromPtrError as NsoFromPtrError, Nso},
    romfs::{
        DirIterator, FromBytesError as RomFsFromBytesError, OpenError as RomFsOpenError, RomFs,
        RomFsDir, RomFsEntry, RomFsFile, RootDirError as RomFsRootDirError,
    },
};
