//! The build identifier shared by the executable formats.
//!
//! NRO, NSO and NPDM all carry the same 32-byte identity, so it is declared once
//! here rather than per format.

/// Identity of a linked binary, as NRO, NSO, and NPDM all record it.
///
/// Taken from the ELF build ID, truncated or zero-padded to `0x20` bytes. The console reports it
/// on a crash, which is what ties a report back to the build that produced the image.
pub type BuildId = [u8; 0x20];
