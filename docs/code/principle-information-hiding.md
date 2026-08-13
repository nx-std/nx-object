---
name: "principle-information-hiding"
description: "Information Hiding — reveal as little as possible; every item takes the most restrictive visibility that works. Load when choosing pub or pub(crate), or reviewing a crate's surface"
type: "principle"
scope: "global"
---

# Information Hiding (Reveal As Little As Possible)

**MANDATORY for ALL code in the workspace**

## Rule

A module is defined by the design decision it **hides**. Its surface reveals as little as possible about how it
works, so the decision can change without any caller learning that it did.

Operationally: **every item carries the most restrictive visibility that still lets it do its job.** Private is
the default, and each widening is a decision that needs a reason:

| Widen to     | When                                    |
|--------------|-----------------------------------------|
| private      | Always, unless something below applies  |
| `pub(crate)` | A sibling module in this crate needs it |
| `pub`        | A consumer outside the crate needs it   |

There is no fourth case. `pub(super)` and `pub(in path)` are not a middle ground — they signal that the module
tree is wrong, because an item exactly one parent may see is an item that belongs to that parent.

Two consequences make this a principle rather than a style rule. **Widening is one-way**: once an item is
`pub`, retracting it is a breaking change, so the cost of guessing wrong is asymmetric — guess small. And
**encapsulation here is module-level, not type-level**: Rust's privacy boundary is the module, so hiding is
achieved by what a module declares and re-exports, not by wrapping fields in accessors.

## Examples

1. **Private by default, gated at the module declaration**
   An item made public "in case someone needs it" is a promise nobody can withdraw. When a whole module is
   internal, say so once where it is declared.

```rust
// ❌ Bad — the `mod` line says the module is public and the restriction is repeated on
// every item. A reader has to check each item to learn what escapes, and the next item
// added defaults to `pub` and quietly escapes.
pub mod blz;

// inside the module
pub(super) fn encode_block(input: &[u8], scratch: &mut Vec<u8>) -> usize {}
pub(super) fn decode_block(input: &[u8], out: &mut Vec<u8>) -> usize {}
pub(super) struct Window { /* ... */ }
pub struct Encoder { /* ... */ } // ...like this one, exported forever by accident
```

```rust
// ✅ Good — the boundary is stated once, at the declaration. Items inside are plain
// `pub`, so the module reads normally, and nothing leaks past the gate. The compressor
// is an implementation detail of how a KIP1 is written, not part of the crate's surface.
pub(crate) mod blz;

// inside the module
pub fn encode_block(input: &[u8], scratch: &mut Vec<u8>) -> usize {}
pub fn decode_block(input: &[u8], out: &mut Vec<u8>) -> usize {}
pub struct Window { /* ... */ }
```

2. **Hide the decision, not just the data**
   A type whose surface mirrors its representation has hidden nothing, however private its fields are.

```rust
// ❌ Bad — the fields are private, but every accessor re-exposes the representation
// one method at a time. Switching from a sorted Vec to a BTreeMap is a breaking
// change to four signatures, which is exactly what encapsulation was meant to prevent.
pub struct RomFsIndex {
    entries: Vec<(EntryPath, FileOffset)>,
}

impl RomFsIndex {
    pub fn entries(&self) -> &[(EntryPath, FileOffset)] {}
    pub fn entries_mut(&mut self) -> &mut Vec<(EntryPath, FileOffset)> {}
    pub fn sort(&mut self) {}
    pub fn binary_search(&self, entry: &EntryPath) -> Result<usize, usize> {}
}
```

```rust
// ✅ Good — the surface is the question callers actually ask. The representation,
// the ordering, and the lookup strategy are all one module's business, and any
// of them can change without a caller noticing.
pub struct RomFsIndex { /* ... */ }

impl RomFsIndex {
    pub fn insert(&mut self, entry: EntryPath, offset: FileOffset) {}
    pub fn offset_of(&self, entry: &EntryPath) -> Option<FileOffset> {}
}
```

## Why It Matters

Parnas's argument has not aged: you decompose a system by what is **likely to change**, and each module hides
one of those decisions. A module that reveals its representation has published a decision instead of hiding
one, and every consumer becomes a reason not to revise it.

The cost is asymmetric in a way that makes caution cheap. Keeping an item private costs one `pub(crate)` the
day a sibling needs it. Making it public costs a major version bump the day you want it back, plus a
coordinated edit across every crate that reached for it in the meantime.

## Pragmatism Caveat

`pub(crate)` is a normal, healthy visibility and needs no defence: internal helpers, shared constants, and
cross-module types live there. It is `pub(super)` and `pub(in path)` that are the smell — reach for one and the
question to ask is whether the item belongs in the module it is being shown to.

Test-only access is not a reason to widen. A private item is testable from a `#[cfg(test)]` module inside the
same file or module; widening it so an integration test can reach it exports an implementation detail to every
consumer in order to satisfy one test.

The rule is about what a module **reveals**, not about ceremony. A plain data type with no invariant — a
`#[repr(C)]` header a caller maps zero-copy onto a byte slice, a spec deserialized at the edge — may have
public fields, because no decision is being hidden and accessors would add nothing but noise. Where an
invariant does exist, the field stays private and the constructor enforces it. That is why a layered crate can
be both: the `raw` layer is public down to its fields because zero-copy access _is_ its contract, while `read`
and `write` re-export a curated set of names from their submodules and keep the rest — scratch buffers,
compression helpers, partially built state — private.

When you deliberately expose more than a caller strictly needs — a field a downstream crate reads on a hot
path, an internal type a macro must name — say why at the declaration. An undocumented `pub` on something the
crate does not intend to support is indistinguishable from an oversight.

## Checklist

Before committing code, verify:

- [ ] Every item is private unless a specific caller requires otherwise
- [ ] `pub(crate)` is used for cross-module needs inside the crate; `pub` only for items a consumer outside
      the crate calls
- [ ] No `pub(super)` or `pub(in path)` was introduced; where one felt necessary, the module tree was
      reconsidered instead
- [ ] A module that is internal is gated at its `mod` declaration, not by annotating each of its items
- [ ] A type's public surface is the questions callers ask, not a mirror of its representation
- [ ] No item was made public solely so a test could reach it
- [ ] Public fields appear only on types with no invariant to protect
- [ ] Any deliberate over-exposure is documented at the declaration

## References

- [principle-single-responsibility](principle-single-responsibility.md) - Related: A module can only own one
  responsibility if callers cannot reach past its surface
- [principle-open-closed](principle-open-closed.md) - Related: Internals can only be restructured freely while
  nothing depends on them
- [principle-law-of-demeter](principle-law-of-demeter.md) - Related: A surface that reveals its internals is
  what makes reach-through possible
- [principle-type-driven-design](principle-type-driven-design.md) - Related: A private field is what lets a
  constructor be the only way to build a valid value

## External References

- [On the Criteria To Be Used in Decomposing Systems into Modules — D. L. Parnas](https://dl.acm.org/doi/10.1145/361598.361623)
- [Effective Java, Item 15 — Minimize the accessibility of classes and members](https://github.com/clxering/Effective-Java-3rd-edition-Chinese-English-bilingual/blob/dev/Chapter-4/Chapter-4-Item-15-Minimize-the-accessibility-of-classes-and-members.md)
- [Effective Rust, Item 22 — Minimize visibility](https://effective-rust.com/visibility.html)
- [Information Hiding and Encapsulation — David Gries](https://www.cs.cornell.edu/courses/JavaAndDS/files/infoHiding.pdf)
- [Least Privilege Principle — OWASP](https://owasp.org/www-community/controls/Least_Privilege_Principle)
