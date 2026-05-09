//! High-level parsing wrappers for Nintendo Switch executable formats.
//!
//! This module provides safe, validated parsing interfaces over the raw binary structures.
//! Each parser validates magic numbers, sizes, and provides convenient accessor methods.

mod mod0;
mod nacp;
mod npdm;
mod nro;
mod nso;
mod romfs;

pub use self::{
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
