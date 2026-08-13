---
name: "rust-mods-files"
description: "Module file layout without mod.rs: a named module file beside its directory, declaring its sub-modules and re-exporting their public types. Load when creating a module, adding a sub-module, or splitting a file that has grown"
type: core
scope: "global"
---

# Module Files

**MANDATORY for ALL Rust code in this workspace**

Where a module's files sit and what the named module file carries. The invariants behind these rules are
stated in [rust-mods](rust-mods.md).

## 1. Never Use `mod.rs`

**DO NOT** use `mod.rs` files. A module with sub-modules is a named file sitting next to a directory of the
same name.

```
// ❌ Bad — every module is a file called mod.rs, so an editor's tab bar and a stack trace
// both say "mod.rs" and neither says which module you are looking at.
src/
  read/
    mod.rs         // Read module code and sub-module declarations
    nro.rs
    romfs.rs
```

```
// ✅ Good — the file name is the module name, so it is findable by search and unambiguous
// in a trace.
src/
  read.rs          // Read module code and sub-module declarations
  read/
    nro.rs
    romfs.rs
```

Two kinds of file keep `mod.rs`, and neither is production source:

- **Test modules.** Every top-level file in a `tests/` directory compiles as its own test binary, so a module
  shared between suites has nowhere outside the directory to be declared from. Test module trees therefore use
  `mod.rs`, in `tests/` and in `#[cfg(test)]` trees under `src/` alike.
- **Generated code.** A generator emits the tree it emits, and a checked-in generated directory is not
  restructured by hand.

Nothing else qualifies. A `mod.rs` in production source is a violation however deep in the tree it sits.

## 2. The Module File Declares Its Children and Re-Exports Them

The named module file declares its sub-modules and re-exports their public types; sub-module files carry one
concern each.

```rust
// ✅ Good — callers depend on `read::Nro`, so moving `Nro` between sub-modules stays an
// edit to one re-export line rather than a change at every call site.

// In src/read.rs
mod nro;
mod romfs;

pub use self::{
    nro::Nro,
    romfs::RomFs,
};

// In src/read/nro.rs
/// High-level NRO parser with segment and asset access.
#[derive(Debug)]
pub struct Nro<'a> { /* ... */ }
```

The re-export is the module's public surface. A type a caller outside is meant to name is re-exported here; a
type only the sub-modules use is not.

Code the module file carries beyond its declarations and re-exports is reachable only downward, never from
the sub-modules it declares ([rust-mods-graph](rust-mods-graph.md)).

## Checklist

Before committing Rust code, verify:

- [ ] No `mod.rs` file exists in production source; the only ones are in test trees and generated directories
- [ ] Named module files (e.g. `read.rs`) sit next to directories of the same name
- [ ] Sub-modules are declared with `mod` in the parent module file
- [ ] Types callers outside the module need are re-exported from it with `pub use self::`

## References

- [rust-mods](rust-mods.md) - Extends: The invariants these rules make operational
- [rust-mods-graph](rust-mods-graph.md) - Related: Which references between these files are legal
- [rust-mods-members](rust-mods-members.md) - Related: Order of the items inside one module file
- [rust-imports](rust-imports.md) - Related: Owns the prologue and the `self::` re-export form
