---
name: "rust-docs-rustdoc"
description: "Rustdoc: crate, module and item levels; mandatory # Panics and # Errors; never # Returns, # Arguments or # Examples. Load when writing a /// or //! block"
type: "core"
scope: "global"
---

# Rustdoc

**MANDATORY for ALL Rust code in the workspace**

Rustdoc is the consumer's channel ([rust-docs](rust-docs.md)): the hover text at every call site and the
rendered API page. It carries contracts and domain, in the timeless present. History, process, and the
argument for an exception belong in a `//` comment ([rust-docs-comments](rust-docs-comments.md)).

## 1. Three Levels, Three Questions

Each level answers a different question, and a block that answers the wrong one is in the wrong place.

| Level      | Written as            | Answers                                                        |
|------------|-----------------------|----------------------------------------------------------------|
| **Crate**  | `//!` in `src/lib.rs` | What is this crate for, and why would I depend on it?          |
| **Module** | `//!` at the top of a module | Why does this module exist, and what does it defend against? |
| **Item**   | `///` on the item     | What does calling this give me, and what must I uphold?        |

```rust
// ✅ Good — the crate root, in the terms of someone deciding whether to depend on it.
//! Zero-copy parsing and generation of Nintendo Switch executable formats.
//!
//! Turns a byte buffer into a validated view of an NRO, NSO, NACP, NPDM, or RomFS
//! image, and builds each of those formats back out of its parts.

// ✅ Good — the module says why it exists and what it protects, which is the fact
// the next editor most needs and no signature carries.
//! BLZ compression for KIP1 segments.
//!
//! Both encoding and decoding run from the end of the data towards the start,
//! because the loader decompresses in place: a decoder writing back-to-front
//! never overwrites compressed bytes it has not yet read.
```

A module `//!` block is where an invariant that the module relies on, or upholds, is stated. That is a fact
about this code, so it belongs here rather than in a rule document: the reader who needs it is editing this
file.

## 2. Say the Contract, Briefly

Every public item carries a description: one or two sentences, contract first. Non-obvious behavior — edge
cases, ordering, atomicity — earns another sentence. Nothing else does.

```rust
// ❌ Bad — a paragraph per parameter, restating the signature in prose. It says
// nothing a reader cannot see, and it goes stale the moment an argument moves.
/// Adds a file to the RomFS image
///
/// # Arguments
/// * `path` - The entry path under which the file is stored in the image
/// * `contents` - The file data structure containing all bytes to be written
pub fn add_file(&mut self, path: EntryPath, contents: Vec<u8>) {}

// ✅ Good — the contract, then the one behavior a caller has to schedule around.
/// Add every file under `dir`, in one pass. A failure leaves the builder untouched.
pub fn add_dir(&mut self, dir: &Path) -> Result<(), ScanError> {}
```

`# Arguments` is never written. A parameter that needs explaining needs a better name or a type that carries
the meaning ([pattern-newtype](pattern-newtype.md)).

## 3. Sections That Are Never Written

- **`# Returns`** — the return type says it. A sentence restating `-> Config` as "returns the current
  configuration" is noise that survives until the signature changes and then becomes wrong.
- **`# Arguments`** — see above.
- **`# Examples`** — usage examples are what tests are for, and a hand-written example compiles only until it
  doesn't. The exception is a doctest that **pins a contract**: [rust-fmt](rust-fmt.md) requires one on every
  formatting impl, because the exact rendering is the promise and an assertion is the only form of it that
  cannot drift. A doctest asserting a contract is required; one demonstrating typical usage is not written.

Every one of those bans rests on the same premise: the reader is holding the signature. The return type is
visible, the parameter names are visible, and a section restating them adds a second place for the same fact
to rot.

**Where the reader has no signature, the premise fails and the sections come back.** An item whose rustdoc is
**product surface** — read by someone driving the tool rather than calling it from Rust — has a reader with no
types to consult and no parameter names to improve. The NPDM descriptor types are the case this workspace has:
their rustdoc is rendered into the generated JSON schema under `docs/schemas/`, and the person reading it is
authoring an NPDM manifest by hand. There, a field's doc is the only place its accepted values and units can
live. Such items state what a schema reader needs, whatever this section otherwise bans.

The test is not "is this public" but **"can the reader see the signature?"** For ordinary Rust items the
answer is yes, and the bans stand.

```rust
// ❌ Bad — a usage demo, duplicating a test and rotting on the next signature change.
// `/// Validate an entry path against the RomFS naming rules.` is the whole doc needed.
/// Validates an entry path
///
/// # Examples
/// ```
/// assert!(validate_entry_path("romfs/config.json").is_ok());
/// ```
pub fn validate_entry_path(path: &str) -> Result<(), Error> {}
```

## 4. Sections That Are Mandatory

**`# Panics`** — any function that can panic says so, and names the condition. That includes a function whose
body reaches an `unwrap`, an `expect`, a `panic!`, an indexing operation on a data-derived index, or a call to
something that panics.

```rust
// ✅ Good — the condition, not the mechanism.
/// The end offset of the last segment in `segments`.
///
/// # Panics
///
/// Panics if `segments` is empty.
pub fn end_offset(segments: &[SegmentBounds]) -> u32 {}
```

The section documents a panic a caller can actually reach. An `unwrap` or `expect` that a code invariant makes
unreachable is the exception: it carries a `// SAFETY:` comment on its statement, stated where the call sits,
and gets **no** `# Panics` section — documenting a panic that cannot occur would mislead the caller. That
comment and the proof behind it are owned by [rust-errors-handling](rust-errors-handling.md).

**`# Errors`** — any fallible public function describes what its failures mean. The variants themselves are
documented on the error type, which [rust-errors-reporting](rust-errors-reporting.md) governs; this section
says which of them a caller can expect here, and what they imply.

**`# Safety`** — required on every constructor that skips validation. Those rules, including the `// SAFETY:`
comment each call site carries, are owned by [rust-fn-unchecked](rust-fn-unchecked.md).

Two adjacent requirements live elsewhere and are not restated here: the documentation template for error enums
and their variants ([rust-errors-reporting](rust-errors-reporting.md)), and the `Cargo.toml` feature comment
([rust-crates](rust-crates.md)).

## Checklist

Before committing code, verify:

- [ ] The crate root `//!` says what the crate is for, in the terms of someone deciding whether to depend on it
- [ ] Every module has a `//!` block saying why it exists and stating any invariant it relies on or upholds
- [ ] Every public item has a one-or-two-sentence description, contract first
- [ ] No `# Returns`, `# Arguments`, or usage-demo `# Examples` section was added
- [ ] Any doctest present pins a contract rather than demonstrating typical usage
- [ ] Every function that can panic has a `# Panics` section naming the condition
- [ ] A provably-unreachable `unwrap`/`expect` has a `// SAFETY:` comment on its statement and no `# Panics`
      section
- [ ] Every fallible public function has an `# Errors` section saying what its failures mean
- [ ] Every validation-skipping constructor has a `# Safety` section

## References

- [rust-docs](rust-docs.md) - Extends: The intent rule, the audience routing, and the shared voice, applied to
  the consumer's channel
- [rust-docs-comments](rust-docs-comments.md) - Related: The editor's channel, and the leading comment that
  sits after a doc comment
- [rust-fn-unchecked](rust-fn-unchecked.md) - Related: Owns `# Safety` sections and their call-site comments
- [rust-errors-handling](rust-errors-handling.md) - Related: Owns the `// SAFETY:` comment on a
  provably-unreachable unwrap/expect, which stands in for the omitted `# Panics` section
- [rust-fmt](rust-fmt.md) - Related: The one place a doctest is mandatory, because the rendering is the contract
- [rust-errors-reporting](rust-errors-reporting.md) - Related: Owns the error enum and variant documentation template
- [rust-crates](rust-crates.md) - Related: Owns `Cargo.toml` feature documentation
