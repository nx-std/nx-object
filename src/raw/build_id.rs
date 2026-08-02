//! The build identifier shared by the executable formats.
//!
//! NRO, NSO and NPDM all carry the same 32-byte identity, so it is declared once
//! here rather than per format.

/// 32-byte build ID / module ID used in NRO, NSO, and NPDM formats.
pub type BuildId = [u8; 0x20];
