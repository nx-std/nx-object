---
name: "rust-docs-todo"
description: "TODO comments: one keyword, optional GitHub handle owner, one-space continuation, placed between rustdoc and the item. Load when leaving or reviewing deferred work"
type: "core"
scope: "global"
---

# TODO Comments

**MANDATORY for ALL Rust code in the workspace**

A TODO is the one comment that is not about the code in front of you: it is about **work that is not there
yet**. That makes it project process, so it lives in the editor's channel, a `//` comment, and it must survive
being read by someone who was not in the conversation that produced it.

## 1. Anatomy of a TODO

```
// TODO(handle): what must be done, in the imperative, and enough of why to act on it.
```

The keyword is uppercase `TODO`, the parenthetical is optional, the colon is not, and the text is a **complete
sentence stating the work** — not a mood about the code. The reader who acts on it is a stranger: they need the
task and the reason it is not done, because the reason is what tells them whether the moment has arrived.

```rust
// ❌ Bad — the first names a feeling, not a task: nobody can act on it and nobody
// can close it either, so it lives forever. The second is a task with no reason, so
// the next reader cannot tell whether it was deferred for a good cause or simply
// forgotten, and leaves it alone.
// TODO: this is ugly
// TODO: split this module

// ✅ Good — the task, the evidence, and the cost of leaving it, in one sentence a
// stranger can act on.
// TODO(dana): split this module along its four sections — the section banners
//  below are the module boundaries it is missing, and a consumer that needs one
//  section compiles all four.
```

A TODO is a **debt**, and it is written to be paid. If the work is not worth describing precisely enough for
someone else to do it, it is not worth a comment; delete the thought or open an issue.

## 2. The Parenthetical Is a GitHub Handle

The optional parenthetical holds **one thing: the GitHub handle of the individual who owns the TODO and will
address it.** There are exactly two forms:

| Form             | Means                                        | Use when                                       |
|------------------|----------------------------------------------|------------------------------------------------|
| `TODO(handle)`   | This person owns the debt and will settle it  | Someone has accepted the work                  |
| `TODO:`          | Unowned                                       | The work is small, local, and obvious in place |

The handle is written **bare, without the `@` sigil**: `TODO(dana)`, not `TODO(@dana)`. The parentheses already
mark it, and a sigil inside a source comment mentions nobody — it only invites a search that will not find it.

Only a **user** handle. Not a crate, a module, a subsystem, a ticket, a category, or a team or org handle
(`org/team`): all of those read like ownership without conferring any, and neither a place nor a group picks up
work — a person does. If nobody has taken the debt, write the bare `TODO:` and let the sentence carry it.

```rust
// ❌ Bad — the parenthetical names an area, so the TODO looks owned and is not.
// Everyone working in the crate assumes someone else is on it.
// TODO(nx-object): NSOs with a compressed `.data` segment are unsupported.

// ❌ Bad — a team handle spreads the debt across a group, which means nobody is
// answerable for it. It will outlive every person who was on that team.
// TODO(nx-std/nx-object): NSOs with a compressed `.data` segment are unsupported.

// ✅ Good — a handle, so the debt has one person's name on it, and a reviewer knows
// exactly who to ask whether it is still deferred on purpose.
// TODO(dana): NSOs with a compressed `.data` segment are unsupported — the
//  segment is skipped with a warning rather than decompressed.

// ✅ Good — unowned, because the work is small, local, and obvious in place.
// TODO: drop this alias once the last caller moves to `read_segment`.
```

A handle is a claim on someone's time, so it is written **only** with their agreement. Naming someone who never
accepted the work produces a comment that is read once, resented, and ignored — worse than no owner at all,
because it stops anyone else from picking the work up.

## 3. Multiline TODOs Indent One Space

A TODO that needs more than one line continues on `//` lines whose text is indented **one space past the first
line's text**. That indent is not decoration: IDE TODO tooling parses an item by following the indentation, so
a continuation line that is not indented starts a **new** item, filing the tail of the sentence as a second,
truncated task in the TODO window. One space is enough, on every continuation line, not just the second.

```rust
// ❌ Bad — flush continuation, read as two TODO items, the second of which
// ("skipped with a warning…") is a fragment that means nothing on its own.
// TODO(dana): NSOs with a compressed `.data` segment are unsupported —
// the segment is skipped with a warning rather than decompressed.

// ✅ Good — one extra space on every continuation line: the whole sentence folds
// into a single TODO item, and the shape reads as a hanging indent.
// TODO(dana): NSOs with a compressed `.data` segment are unsupported —
//  the segment is skipped with a warning rather than decompressed, because the
//  reader handles uncompressed segments only.
```

## 4. TODO Is the Only Keyword

`FIXME`, `XXX`, and `HACK` are **not used**. They fragment the same search across four spellings while adding
nothing a sentence cannot say: a TODO whose text names a defect _is_ a FIXME, and its text says how bad it is
far more precisely than a keyword ever could. One keyword means a search for `TODO` is the complete debt list,
and a complete list is the only kind anyone trusts.

```rust
// ❌ Bad — a second keyword for the same idea. Now a search for TODO misses it,
// and FIXME carries no severity a sentence could not carry better.
// FIXME: compressed .data segments get dropped

// ✅ Good — one keyword, and the severity lives in the sentence, which says what a
// caller actually loses.
// TODO(dana): NSOs with a compressed `.data` segment are unsupported —
//  the segment is skipped with a warning rather than decompressed, so a build
//  against one produces an executable missing its data.
```

## 5. A TODO Never Goes in Rustdoc

**No TODO appears in a `///` or `//!` block.** Rustdoc is the consumer's channel, rendered on every doc page and
shown in every hover, and the reader there cannot act on the project's backlog.

The placement is fixed: **after the rustdoc block and immediately before the item it concerns**. That position
makes it a comment about unfinished work rather than part of the item's documented contract, and keeps the debt
visible to the person editing the code without leaking it to everyone reading the docs.

```rust
// ❌ Bad — backlog in the rendered docs. The caller cannot act on it, and it shows
// up on the doc page and in every hover until someone does.
/// Collect the segment bounds declared by an NSO header.
///
/// TODO(dana): segments with a compressed `.data` are skipped.
pub fn segments(nso: &Nso) -> Vec<SegmentBounds> {}

// ✅ Good — the rustdoc describes the function; the TODO sits between the docs and
// the item, where the next person to edit this sees it and no consumer does.
/// Collect the segment bounds declared by an NSO header.
// TODO(dana): NSOs with a compressed `.data` segment are unsupported — the
//  segment is skipped with a warning rather than decompressed, so a caller gets
//  bounds that omit it.
pub fn segments(nso: &Nso) -> Vec<SegmentBounds> {}
```

If the unfinished work changes what a caller can rely on today, that is a fact about the API and it belongs in
the rustdoc **as documentation** — a sentence describing the current behavior, not a TODO. Write what the
function does now; leave the debt in the comment below it.

## 6. A TODO Is Not a Justification

Several rules in this workspace require a written reason at the point of the exception: a `// SAFETY:` comment
above an unchecked constructor, a `reason = "..."` on a lint suppression, a comment explaining a deliberate
reach-through or a swallowed error. A TODO satisfies none of them. A justification explains why the code is
**correct as it stands**; a TODO says the code is **not finished**. Replacing the first with the second removes
the argument a reviewer needs and leaves a promise nobody is tracking.

```rust
// ❌ Bad — the TODO stands in for the SAFETY comment. A reviewer is left without
// the reason the invariant holds, and the unchecked call ships unexamined.
// TODO: validate this properly
Ok(EntryPath::from_ref_unchecked(name))

// ✅ Good — the justification says why this is correct now. A TODO may sit
// alongside it, but it does not replace it.
// SAFETY: `RomFsBuilder` rejects absolute, empty, and `..`-bearing paths before
// writing the name table, so every name read back upholds the invariants.
Ok(EntryPath::from_ref_unchecked(name))
```

## Checklist

Before committing code, verify:

- [ ] Every TODO states the work in the imperative, with enough of the reason that a stranger can act on it
- [ ] The parenthetical, if present, is an individual's GitHub handle and nothing else — not a crate, module,
      category, ticket, or team/org handle
- [ ] The handle is written bare, without the `@` sigil
- [ ] A TODO naming a handle was agreed with that person; otherwise it is written unowned as `TODO:`
- [ ] Every continuation line of a multiline TODO is indented one space past the first line's text
- [ ] The keyword is `TODO`; no `FIXME`, `XXX`, or `HACK` was introduced
- [ ] No TODO appears inside a `///` or `//!` block; it sits between the rustdoc and the item it concerns
- [ ] No TODO stands in for a `// SAFETY:` comment, a lint `reason`, or any other required justification

## References

- [rust-docs-comments](rust-docs-comments.md) - Extends: The editor's channel a TODO lives in, and the
  justification rules it does not replace
- [rust-docs-rustdoc](rust-docs-rustdoc.md) - Related: The consumer channel a TODO never enters
- [rust-fn-unchecked](rust-fn-unchecked.md) - Related: The `// SAFETY:` comment a TODO does not replace
- [rust-attrs-lints](rust-attrs-lints.md) - Related: The `reason` on a suppression, which is a justification
  rather than a debt
- [rust-errors-handling](rust-errors-handling.md) - Related: Swallowed errors need a justification, not a TODO
