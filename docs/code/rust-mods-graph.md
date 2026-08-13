---
name: "rust-mods-graph"
description: "The reference graph between modules: no reference into the file that declared you, no sibling cycles. Load when splitting a module, or when a sub-module reaches back into its parent"
type: core
scope: "global"
---

# Module Reference Graph

**MANDATORY for ALL Rust code in this workspace**

Which references between the modules of a crate are legal. The invariant behind these rules is stated in
[rust-mods](rust-mods.md); where the files themselves sit is owned by
[rust-mods-files](rust-mods-files.md).

## 1. A Sub-Module Does Not Reach Into the File That Declares It

The module file **may** hold code, and what it holds is unavailable to its children. A sub-module refers to
its siblings and to items addressed from the crate root, never to an item declared one level up.

```rust
// ❌ Bad — the NRO reader cannot be read, moved, or tested without its parent, and the two
// files now import each other: the parent calls the reader, the reader names the parent's
// type. Splitting `read.rs` was supposed to make the pieces separable and did not.

// In src/read.rs
mod nro;

pub struct SegmentBounds { offset: usize, size: usize }

// In src/read/nro.rs
use super::SegmentBounds;
```

```rust
// ✅ Good — the type sits in a sub-module of its own, the parent re-exports it for callers
// outside, and the reader names a sibling. Every file under `read/` reads on its own.

// In src/read.rs
mod nro;
mod segment_bounds;

pub use self::segment_bounds::SegmentBounds;

// In src/read/nro.rs
use super::segment_bounds::SegmentBounds;
```

A `use super::Item` naming a type, constant, or function rather than a sibling module is the signal. The fix
is always the same: the item moves down into a sub-module, and the parent re-exports it if callers outside
need the name.

## 2. Siblings Are a Legal Edge; a Cycle Is Not

A sub-module reaching a sibling goes up one level and back down: `use super::sibling::Item`. That edge is
legal, and it is the reason "no upward references" is not the whole rule, because two siblings can reference
each other and close a loop without either one reaching for its parent.

```rust
// ❌ Bad — `nro` names `mod0::Mod0` and `mod0` names `nro::Nro`.
// Neither file reaches into the parent, and the pair is still unreadable in either order.

// In src/read/nro.rs
use super::mod0::Mod0;

// In src/read/mod0.rs
use super::nro::Nro;
```

Break the loop by moving what both need into a third sibling that neither of them imports from, or by
merging the two: a pair of modules that each need the other's types is usually one concern that was split
along the wrong seam.

The shape of these paths, and the ban on `super::super::`, is owned by [rust-imports](rust-imports.md).

## Checklist

Before committing Rust code, verify:

- [ ] Nothing a sub-module needs is declared in the file that declares that sub-module
- [ ] No `use super::Item` names anything but a sibling module
- [ ] No pair of sibling modules imports from each other
- [ ] No chain of references leads from a module back to that same module

## References

- [rust-mods](rust-mods.md) - Extends: The one-way invariant these rules make operational
- [rust-mods-files](rust-mods-files.md) - Related: Where the module files these references run between sit
- [rust-imports](rust-imports.md) - Related: Owns the path form, the `self::` prefix, and the `super::super::` ban
