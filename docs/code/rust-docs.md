---
name: "rust-docs"
description: "The two doc channels — rustdoc for consumers, // for editors — audience routing, intent over implementation, shared voice. Load when writing any doc comment or comment"
type: "core"
scope: "global"
---

# Rust Documentation Style

**MANDATORY for ALL Rust code in the workspace**

One rule outranks every other here, and the rest are consequences of it: **always document intent, never
implementation — and when you record a change, record the logical change, not the code change**
([§4](#4-intent-over-implementation)).

## 1. Two Channels, Two Audiences

Source carries two documentation channels, and they are not interchangeable:

- **Rustdoc** (`///` on an item, `//!` on a module or crate root) is read by the code's **consumer**: the hover
  text at every call site, the rendered API page, the crate's front door. Its three levels — crate, module,
  item — each answer a different question.
- A **comment** (`//`) is read by the code's **editor**: the person with the file open, about to change the
  line the comment sits on. A `// SAFETY:` note and a lint `reason` are comments held to a higher bar.

The channel is chosen by **audience**, never by length: a one-sentence fact can be rustdoc, a five-line
argument can be a comment.

This document is the **base** of the `rust-docs-*` group: it owns the audience rule, the intent rule, and the
voice both channels share. Each channel's own rules live in a doc that extends this one, discovered by the
shared prefix rather than by an index here that would go stale on the first rename.

## 2. Route by Audience

Ask who needs the information, and it files itself:

| The information                                     | Its home                                          |
|-----------------------------------------------------|---------------------------------------------------|
| What the crate is for, and why depend on it         | Crate root `//!`                                  |
| Why the module exists, what it defends against      | Module `//!`                                      |
| What an item does, and the contract a caller gets   | Item `///`                                        |
| The default of an optional config field             | Field `///`, with the literal value               |
| The conditions under which a function panics        | `# Panics` section                                |
| What a caller must uphold at an unchecked call      | `# Safety` section, and `// SAFETY:` at the call  |
| Why this line is shaped this way, what breaks else  | Comment at the line                               |
| What the code used to be, and what that cost        | Leading comment — or only the commit message      |
| What was verified against an upstream source        | Leading comment on the code it defends            |
| Why a lint is suppressed                            | `reason = "..."` on the `#[expect]`               |
| Work that is deferred, and who owes it              | `TODO` comment                                    |
| How a design was decided                            | `README.md`, the design notes under `docs/`       |
| Why this change was made                            | The commit message                                |

The two channels compose at a single declaration:

```rust
// ✅ Good — the rustdoc states the contract a caller relies on; the comment
// recounts, for the next editor, the shipped mistake the shape guards against.
/// Lay out the `text`, `rodata`, and `data` segments of an NRO.
///
/// Every segment starts on a page boundary: the loader maps each one with its own
/// permissions, so a segment sharing a page with the previous one is mapped with
/// the wrong flags.
// This packed the segments tightly to save a few kilobytes, and the resulting NROs
// faulted on the first `rodata` read — on hardware only, since the emulator maps
// everything readable.
pub fn plan_segments(elf: &Elf) -> Vec<SegmentBounds> {}
```

The corollary is a tense rule: **rustdoc describes the code as it is**, in the timeless present. Only a comment
may recount the past — a "used to be" in rustdoc is misrouted history, rendered to every consumer and serving
none of them. Status is the same: no "phase 1 implemented, phase 2 pending" in rustdoc, which describes a
schedule rather than the API.

## 3. A Rule Document Is Never Cited as Authority

No sentence in source appeals to `docs/code/` to justify the code beneath it — not in rustdoc, not in a
comment. A rule-document citation is process, not domain: it tells the consumer nothing, and as a comment it
rots the moment the corpus is reorganized. When a sentence genuinely needs the convention, name it in prose
that makes a claim — the corpus is discovered by name.

```rust
// ❌ Bad — a path into the code rules, standing in for the argument, and
// rendered on every docs page.
/// A RomFS entry path known to be well-formed.
///
/// See docs/code/pattern-newtype.md.
pub struct EntryPath(String);

// ✅ Good — names the mechanism and why the line needs it; no path.
// The invariant is established in `FromStr`, so this wrapper cannot be built from
// an unvalidated string anywhere in the crate.
pub struct EntryPath(String);
```

The rule is about **appeal to authority, not about the string**. Two cases fall outside it rather than through
it: a module whose domain _is_ documentation tooling names those paths as data, and must; and design documents
(`README.md`, the notes under `docs/`) describe behavior, which _is_ domain, so rustdoc may cite them.

## 4. Intent Over Implementation

**Always document intent; never document implementation.** When a sentence could say what the code _does_ or
what it is _for_, it says what it is for. This breaks every tie in both channels, and it is the only rule that
survives the next edit: a sentence about the body is either rewritten with the body or left behind as a lie.

The line is not "high-level versus low-level" — it is **what the reader is entitled to depend on**. A mechanism
named in rustdoc becomes a promise the crate has to keep; a mechanism named in a comment becomes a claim the
next commit can falsify. Mechanism therefore earns its place exactly when a decision the reader must make turns
on it, never as a report of how the body happens to work. The test: would this sentence stop a reasonable
person from "simplifying" the code back into the bug?

```rust
// ❌ Bad — the mechanism is the whole sentence. It breaks the moment the queue
// stops being a VecDeque, and it never tells the caller the one thing they have
// to know: that `enqueue` returns before anything reaches the console.
/// Push the chunk onto the pending deque and start a send task if one is not
/// already running.
pub fn enqueue(&self, chunk: Chunk) {}

// ✅ Good — the guarantee first, and the mechanism only where the caller's own
// design turns on it.
/// Append a chunk to the transfer without ever blocking the caller.
///
/// `enqueue` is a synchronous hand-off; the send happens on a background task.
/// If the console is unreachable, chunks queue in memory (bounded, drop-oldest)
/// and the task retries — the caller never waits, and never sees the failure.
pub fn enqueue(&self, chunk: Chunk) {}
```

### Logical Changes, Not Code Changes

A comment that records **history** records the _logical_ change — what the code now guarantees that it did not
before — never the _code_ change, which is the diff's job and the commit message's. The logical fact is still
an argument against the same mistake a year from now; the diff is not.

```rust
// ❌ Bad — a changelog entry in the source. It records the edit instead of the
// reason the edit was forced, so it stops nobody from doing it again.
// Changed plan_segments to take an Elf instead of a byte slice, and moved the
// call above the header write.
```

## 5. The Shared Voice

Both channels are **prose**: complete sentences, aimed at a specific reader, carrying a claim that reader could
doubt.

- **The smallest true statement.** One line is enough when one line is the whole truth, and silence is enough
  when the code already says it — a comment restating the declaration beside it is noise.
- **Never narrate.** A sentence that paraphrases the next line will be wrong after the next edit and was
  useless before it.
- **Spell values as the code spells them.** `Default 10_000`, not `10000` — a search for the number must find
  both the doc and the code.
- **Backtick every identifier and value.** Rustdoc renders as markdown, so `` `SegmentBounds` ``,
  `` `10_000` ``, and `` `default-features = false` `` are set as code while the prose around them stays
  prose; a bare identifier reads as a word and is invisible to a scan.
- **Link with intra-doc links.** `[`SegmentBounds`]` resolves and stays correct through a rename; a plain-text
  type name does not.

```rust
// ✅ Good — says why, and what breaks otherwise, instead of narrating the line.
// 0x4000 is the largest chunk the console's receive buffer accepts in one read:
// anything larger is split across two reads, and the second is interpreted as a
// new length prefix, which desynchronizes the rest of the transfer.
const MAX_FILE_CHUNK_SIZE: usize = 0x4000;
```

## Checklist

Before committing code, verify:

- [ ] Every doc comment and comment states intent — the guarantee, invariant, or failure mode — and names a
      mechanism only where a decision the reader must make turns on it
- [ ] History records the logical change, never the code change
- [ ] Information sits in its channel: contracts and domain in rustdoc; process, history, and justification in
      comments
- [ ] Rustdoc reads in the timeless present; only comments recount the past, and no doc comment states
      implementation status
- [ ] No sentence cites `docs/code/` as authority for the code beneath it; conventions are named in prose that
      makes a claim
- [ ] Every doc comment and comment is a prose claim a reader could doubt — nothing narrates, nothing restates
      a declaration
- [ ] Values and identifiers are spelled as the code spells them, in backticks, and types use intra-doc links

## References

- [rust-mods-members](rust-mods-members.md) - Related: Where doc comments and comments sit in a file's
  member order
- [principle-least-surprise](principle-least-surprise.md) - Foundation: Documentation exists to stop the next
  reader from "fixing" something on purpose
