---
name: "rust-docs-comments"
description: "Editor-facing // comments: placement, justifying discarded errors and lossy casts, history, no section separators. Load when writing a // comment or discarding an error"
type: "core"
scope: "global"
---

# Rust Comments

**MANDATORY for ALL Rust code in the workspace**

A `//` comment is the editor's channel ([rust-docs](rust-docs.md)): it addresses the person about to change the
code it accompanies, justifying a decision they would otherwise undo or recounting the past they are about to
repeat. It is never rendered to a consumer, so it carries what rustdoc must not: history, provenance, and the
argument for an escape hatch.

## 1. A Comment Accompanies Code

Every comment is attached to a specific piece of code and makes a claim about it. Two placements exist:

- **Inline / at the line** — immediately above the statement it justifies.
- **Leading** — a block of prose above a declaration, **after the doc comment** when the item has one: `///`
  first (the consumer's view), then the `//` comment (the editor's), then the code. At module level the note
  sits below the `//!` block, before the imports. [§3](#3-history-lives-here-or-in-the-commit) shows the shape.

A comment that accompanies nothing — a label announcing what the next region of the file contains — is not a
comment ([§5](#5-no-section-separators)). One that restates the declaration beside it (`// The entry path.`
above `path: EntryPath`) is redundant: the declaration already says it, and the doc comment already says why.

## 2. Comments Justify Decisions

The core inline comment explains a decision a reader would otherwise undo. It never narrates the line below it
([rust-docs](rust-docs.md#5-the-shared-voice)).

```rust
// ✅ Good — explains why two conventions that look mismatched actually line up,
// which is exactly the fact a future reader will doubt and "fix".
// The NRO header records segment ends inclusively; our bounds are half-open, so the
// end is one past it. Dropping the `+ 1` silently truncates every segment by a byte.
let bounds = SegmentBounds::try_from((offset, recorded_end + 1))?;
```

Two decisions are **never** allowed to appear bare:

**1. A discarded error.** Every `let _ =`, `.ok()`, `.unwrap_or_default()` on a `Result`, and every `Err(_)`
arm that does not propagate carries a comment saying why losing this error is correct.
[rust-errors-handling](rust-errors-handling.md) owns the requirement; the comment must name what would break if
the error escaped.

```rust
// ❌ Bad — a silent discard. The next reader cannot tell whether this is a
// considered decision or a bug.
let _ = self.stdout.write_all(line).await;

// ✅ Good — says what the discard protects, so the next reader can weigh the cost
// of the error it loses.
// A closed pipe means the user redirected our output and the reader went away; the
// transfer outlives the forwarded stdio on purpose, and failing here would abort a
// deployment that has already landed on the console.
let _ = self.stdout.write_all(line).await;
```

**2. A lossy or unchecked conversion.** Every `as` cast that can truncate, wrap, or change sign carries the
reason it cannot here. Prefer `try_into()` and handle the failure; a bare `as` on a value that is not obviously
in range is indistinguishable from a bug.

```rust
// ❌ Bad — a silent truncation waiting for an NRO larger than 4 GiB.
let length = file_length as u32;

// ✅ Good — names the bound that makes the cast safe, so a reader can check the
// claim rather than the arithmetic.
// `read_len` is bounded by the buffer, which is MAX_FILE_CHUNK_SIZE (0x4000) bytes,
// so this cannot truncate.
let chunk_len = read_len as u32;
```

Two related justifications are owned elsewhere: the `// SAFETY:` comment above an unchecked constructor
([rust-fn-unchecked](rust-fn-unchecked.md)) and the `reason` on a lint suppression
([rust-attrs-lints](rust-attrs-lints.md)). Both follow this document's voice: name the fact that makes the
exception correct, not the fact that an exception was made.

## 3. History Lives Here, or in the Commit

What the code **used to be** is process, and it routes to this channel — never to rustdoc
([rust-docs](rust-docs.md#2-route-by-audience)). Record it as a leading comment when, and only when, the war
story is what stops the next editor from reintroducing the defect; otherwise the commit message already holds
it. A parenthetical works inline when the history is one clause.

```rust
// ✅ Good — the comment is the argument against "simplifying" the alignment back
// out, which is the shortcut a reader who has not been burned will reach for.
/// Append a file to the image, starting at the next `0x10`-aligned offset.
// Entries used to be packed at their natural offset, which the format permits and
// the console's loader does not: every image with an odd-sized file ahead of a
// directory read back with a shifted name table, and only on hardware.
pub fn add_file(&mut self, path: EntryPath, contents: &[u8]) -> Result<(), AddError> {}
```

## 4. Provenance Defends a Design

A claim that was checked against an upstream source ("verified against the format's documentation") exists to stop an
editor from re-litigating the design. It sits as a leading comment on the exact code the verification defends:

```rust
// ✅ Good — the doc comment states the fact; the comment records that it was
// checked, on the code whose shape depends on it.
/// Read the asset section appended after the NRO image.
// Verified against the switchbrew NRO page: the asset header is optional and sits
// at `header.size`, not at the end of the file, so a trailing signature or padding
// must not be treated as part of it.
pub fn assets(&self) -> Option<&NroAssetHeader> {}
```

## 5. No Section Separators

Banner comments that partition a file into titled regions are banned:

```rust
// ❌ Bad — the banners are chapters, and a file with chapters has one reason to
// change per chapter. A consumer that wants the descriptor compiles the builder
// and its scratch buffers too.
// --------------------------------- Descriptors -----------------------------
pub struct NpdmSpec { /* ... */ }

// --------------------------------- Builder ---------------------------------
pub struct NpdmBuilder { /* ... */ }
```

```rust
// ✅ Good — the banner became a module, and its `//!` says what the banner was
// trying to say.
//! NPDM descriptors: what a program declares it needs, and nothing else.

pub struct NpdmSpec { /* ... */ }
```

A separator is not a style problem — it is a **signal**: the banners are the module boundaries the file is
missing. The moment a file needs internal chapters it has more than one reason to change, and each chapter
drags every other into every consumer that touches one
([principle-single-responsibility](principle-single-responsibility.md)). Split the file along its banners
instead of drawing them; a bare one-line label (`// helpers`) is the same smell at smaller scale.

## Checklist

Before committing code, verify:

- [ ] Every comment accompanies specific code and makes a claim about it; none is a bare label
- [ ] Leading comments sit after the doc comment and before the item they annotate
- [ ] Every discarded error (`let _ =`, `.ok()`, a non-propagating `Err(_)` arm) carries a comment naming what
      would break if the error escaped
- [ ] Every lossy `as` cast carries the bound that makes it safe, or is replaced with `try_into()`
- [ ] History appears only where it stops a defect from returning; the rest stays in commit messages
- [ ] Upstream-verification notes sit on the code they defend, not in rustdoc
- [ ] No comment section separators; a file that wants them is split instead

## References

- [rust-docs](rust-docs.md) - Extends: The intent rule, the audience routing, and the shared voice, applied to
  the editor's channel
- [rust-docs-todo](rust-docs-todo.md) - Related: The one comment that is about work rather than about code
- [rust-fn-unchecked](rust-fn-unchecked.md) - Related: Owns the `// SAFETY:` comment this document's voice
  applies to
- [rust-errors-handling](rust-errors-handling.md) - Related: Owns the rule that a discarded error must be commented
- [principle-single-responsibility](principle-single-responsibility.md) - Foundation: A section separator is an
  SRP violation drawn in ASCII
