---
name: "rust-imports"
description: "Module prologue order: doc, std, external, mod declarations, then self/crate/super; extension traits as _. Load when adding imports or declaring submodules"
type: "core"
scope: "global"
---

# Import and Module Declaration Order

**MANDATORY for ALL Rust code in the workspace**

## 1. The Module Prologue

Every module opens with the same five parts, in this order, separated by blank lines:

0. **Module documentation** — the `//!` block, before any item.
1. **`std` imports**.
2. **External crate imports**.
3. **`mod` declarations**, `pub` and private together in one alphabetical block.
4. **Local imports and re-exports** — `use`/`pub use` of `self::`, `crate::`, and `super::`.

The declarations come **before** the local imports because the local imports refer to them: a reader meets the
module's structure first, then what it pulls out of that structure.

```rust
// ❌ Bad — the doc block is a comment, the groups are interleaved, and the mod
// declarations are buried under the imports that depend on them.
// RomFS directory tree.
use crate::RomFsHeader;
mod entry;
use std::collections::HashMap;
pub use self::entry::RomFsEntry;
use zerocopy::FromBytes;
pub(crate) mod hash;
```

```rust
// ✅ Good — documentation, std, external, declarations, then local imports.
//! RomFS directory tree.
//!
//! Walks the directory and file metadata tables that locate each entry within
//! the image.

use std::collections::HashMap;

use zerocopy::FromBytes as _;

mod entry;
pub(crate) mod hash;
mod name;

use crate::raw::romfs::RomFsHeader;

pub use self::{
    entry::RomFsEntry,
    name::EntryName,
};
```

## 2. What rustfmt Does and Does Not Do

The formatter splits `use` statements into the three groups (std, external, local), merges them per crate, lays
out multi-item braces vertically, and sorts `mod` declarations alphabetically — none of it worth arguing about
in review. What it does **not** do is place the `mod` block: item order is preserved as written, so a prologue
with its declarations in the wrong place formats cleanly and stays wrong. Section 1 is the human's part.

## 3. Submodule Types Travel Through `self::`

A type declared in a submodule and re-exported by its parent is referenced from that parent through `self::` —
in the re-export and in any import of it. A bare module name relies on a path resolution the 2018+ editions
dropped; `crate::` for something one level down states a longer path than the truth and breaks when the module
moves.

```rust
// ❌ Bad — the parent reaches for its own child through the crate root. Moving this
// module anywhere in the tree breaks every one of these lines, for no benefit.
mod nro;

pub use crate::read::nro::Nro;
use crate::read::nro::FromBytesError;
```

```rust
// ✅ Good — the child is addressed relative to the parent that owns it.
mod nro;

pub use self::nro::Nro;
use self::nro::FromBytesError;
```

This applies to the private form too: a parent that consumes a submodule type without re-exporting it still
writes `use self::nro::FromBytesError;`.

## 4. Import From the Defining Module

Import an item from the module that **declares** it, not from a module that happens to re-export it. A
re-export is a convenience for consumers outside the crate; inside the crate it hides where an item lives and
produces two paths to the same type in the same codebase.

```rust
// ✅ Good — the path names the defining module. Through the crate root's re-export
// (`use crate::Config;`) nothing says where Config is defined, and a second module
// importing it the other way makes the two look unrelated.
use crate::config::Config;
```

## 5. Siblings Use `super::`; `super::super::` Is Prohibited

A module reaching a sibling goes up one level: `use super::sibling::Item;`. That is the whole allowance for
relative upward paths. Which edges may exist at all — a sibling module yes, an item declared by the parent
file no — is owned by [rust-mods-graph](rust-mods-graph.md).

**`super::super::` is prohibited**, in `use` statements, in inline paths, and in intra-doc links. A path that
climbs two or more levels is unreadable at the use site — the reader has to reconstruct the file's position in
the tree — and it breaks silently when either module moves. Address the item from the crate root instead.

```rust
// ❌ Bad — the reader cannot tell what this names without knowing where the file sits,
// and moving either module changes what it resolves to without a compile error.
use super::super::SegmentBoundsError;

//! [`build`](super::super::NroBuilder) is called by every packaging step.
```

```rust
// ✅ Good — an absolute path inside the crate, readable in isolation.
use crate::write::nro::SegmentBoundsError;

//! [`build`](crate::write::nro::NroBuilder) is called by every packaging step.
```

A `super::super::` that feels necessary is usually a placement problem: the item the deep path reaches for
belongs closer to its users, or the two modules belong under a shared parent.

## 6. Extension Traits Are Imported As `_`

A trait imported only so its methods resolve is imported **without binding its name**:

```rust
use tokio::io::AsyncWriteExt as _;
use zerocopy::FromBytes as _;
```

Two things follow from dropping the name, and both are the point:

- **Same-named traits can coexist.** `use std::fmt::Write; use std::io::Write;` is a hard error (`E0252`), and
  the same collision appears with any pair of `*Ext` traits sharing a name across crates. Imported `as _`, both
  resolve their methods and neither claims the name.
- **The import's liveness is tied to method calls alone.** A named import stays live if the name appears
  anywhere else — a bound, an `impl`, a qualified call — so deleting the last method call leaves it in place,
  meaning something other than what it was added for. With `as _` there is no other way for it to be used, so
  the day the methods go, the compiler reports it unused.

Import the trait **by name** when the name is needed: as a bound (`fn f<R: Read>(..)`), in an
`impl Trait for Type`, or in a qualified call (`<NroHeader as FromBytes>::ref_from_prefix`). Needing the name
is the signal that this is not a method-only import.

```rust
// ❌ Bad — the name is bound but never referenced. It blocks any other `Read` or
// `Write` the module might need, and it will outlive the calls that justified it.
use std::io::Read;
use tokio::io::AsyncWriteExt;

async fn send_chunk(sock: &mut TcpStream, src: &mut std::fs::File) -> io::Result<()> {
    let mut buf = [0u8; MAX_FILE_CHUNK_SIZE];
    let len = src.read(&mut buf)?;
    sock.write_u32_le(len as u32).await
}
```

Both imports above are method-only: written `use std::io::Read as _;` and
`use tokio::io::AsyncWriteExt as _;`, they take no name and nothing but a method call can keep them alive.

## 7. What Not to Import

Not every path needs a `use`. Three cases stay inline:

- **One-off `std` items** used once or twice in a file: spell `std::pin::pin!` and `std::cmp::Ordering` fully
  qualified at the use site rather than importing them. The qualification is the documentation. Two `std`
  modules are never imported from at all, however often they appear: `std::fmt` ([rust-fmt](rust-fmt.md)) and
  `std::str::FromStr` ([rust-parse](rust-parse.md)).
- **Attribute macros**: write `#[tokio::main]`, not an import of the macro.
- **Glob imports**: `use x::*` does not appear in production code. The one accepted glob is `use super::*;` at
  the top of a `#[cfg(test)] mod tests` block, which pulls in the module under test.

```rust
// ✅ Good — qualified at the use site; nothing to look up, and the prologue carries
// no name the rest of the file never mentions again.
fn earliest(a: &NroSegment, b: &NroSegment) -> std::cmp::Ordering {
    a.file_off.get().cmp(&b.file_off.get())
}

#[tokio::main]
async fn main() -> ExitCode {}
```

## Checklist

Before committing code, verify:

- [ ] The module opens with its `//!` documentation, before any item
- [ ] `std` imports, then external crate imports, each in its own group
- [ ] `mod` declarations form one block after the external imports and before the local imports
- [ ] Local `use`/`pub use` of `self::`, `crate::`, and `super::` come last
- [ ] Submodule types are imported and re-exported through `self::`, never through `crate::` or a bare name
- [ ] Items are imported from the module that declares them, not through a re-export
- [ ] A sibling is reached with `super::`; no path contains `super::super::`, including in doc links
- [ ] Traits imported only for their methods are imported `as _`; a trait is imported by name only when the
      name appears in a bound, an `impl`, or a qualified call
- [ ] One-off `std` items and attribute macros are written fully qualified instead of imported
- [ ] No glob import outside `use super::*;` in a test module
- [ ] The file is formatted, so grouping, granularity, and `mod` sorting are the formatter's output

## References

- [rust-mods-files](rust-mods-files.md) - Related: Module file layout; this doc owns the prologue inside each file
- [rust-mods-graph](rust-mods-graph.md) - Related: Which references between modules are legal; this doc owns the form those paths take
- [rust-mods-members](rust-mods-members.md) - Related: Ordering of the items that follow the prologue
- [rust-fmt](rust-fmt.md) - Related: Owns the never-import rule for `std::fmt`
- [rust-parse](rust-parse.md) - Related: Owns the never-import rule for `std::str::FromStr`
- [rust-docs-rustdoc](rust-docs-rustdoc.md) - Related: The `//!` block that opens the prologue
