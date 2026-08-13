//! Turns a linked ELF into the segments the container formats expect.
//!
//! This is the seam between a toolchain's output and this crate's writers: it reads the
//! `PT_LOAD` segments and the build ID, and hands back something an [`NroBuilder`] or
//! [`NsoBuilder`] can be filled from. Nothing here writes a container format itself.
//!
//! [`NroBuilder`]: crate::write::NroBuilder
//! [`NsoBuilder`]: crate::write::NsoBuilder

pub mod segments;

pub use segments::{ElfSegments, ParseError, SectionInfo};
