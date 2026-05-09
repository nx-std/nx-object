//! ELF parsing for NRO/NSO generation.

pub mod segments;

pub use segments::{ElfSegments, ParseError, SectionInfo};
