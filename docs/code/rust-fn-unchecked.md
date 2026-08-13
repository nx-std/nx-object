---
name: "rust-fn-unchecked"
description: "Unchecked constructors that skip FromStr/TryFrom: # Safety docs and a // SAFETY: comment at every call site. Load when adding or reviewing a *_unchecked call"
type: "core"
scope: "global"
---

# Unchecked Constructors

**MANDATORY for ALL Rust code in the workspace**

## 1. Validation Lives in `FromStr` and `TryFrom`

A newtype carries a proof: a value of this type has been checked. That check has exactly one home — `FromStr`
for string input, `TryFrom` for everything else. Every other constructor delegates to it or bypasses it, and
the bypasses are this document's subject.

An unchecked constructor does not weaken the invariant. It **moves the obligation to prove it** from the type
to the caller — sound only when the caller genuinely holds the proof, and only visible when the call site says
so.

These functions are not `unsafe fn`: nothing here risks undefined behavior in the language sense. The `# Safety`
and `// SAFETY:` conventions are borrowed deliberately, because the discipline is the same — an obligation the
compiler cannot check, discharged in writing at the point where it is assumed.

## 2. When an Unchecked Constructor Is Warranted

There are three honest reasons, and they share a shape: the value's invariant was established somewhere the
type cannot see.

1. **Reading back from an artifact that validated on write.** A re-parse pays for a check the write path
   already made, and turns a format mismatch into a decode error at a random read site rather than at the
   boundary that let it in.
2. **Re-wrapping a value taken from an already-valid instance.** Borrowing from an existing newtype, or
   converting between the borrowed and owned forms of one, cannot produce an invalid value.
3. **Literals in tests**, where the value is visible in the same expression and the check would obscure the
   test's actual subject.

Anything else is validation avoidance. If the reason is "this is a hot loop", measure first: parsing a value
that is already correct is rarely what a profile blames.

## 3. Naming and Visibility

The name states both the input and the bypass, so a reader never has to open the definition to know a check was
skipped:

- `from_ref_unchecked` / `from_owned_unchecked` for the borrowed and owned forms of a `Cow`-backed newtype
- `from_i64_unchecked`, `from_bytes_unchecked` for typed sources

Keep the constructor as narrow as its callers allow. A `pub(crate)` unchecked constructor cannot be reached by
a consumer who does not hold the proof; a `pub` one is part of the API and every downstream crate inherits the
obligation.

## 4. The Declaration Carries a `# Safety` Section

The declaration states what the caller must guarantee in terms of the invariant, not "the value must be
valid", which says nothing. The same constructor written as a plain `new` hides the bypass twice over: the
name does not admit it, and a caller reasonably assumes it validates, because every other constructor does.

```rust
// ✅ Good — the name admits the bypass and the docs name the obligation being transferred.
impl<'a> EntryPath<'a> {
    /// Create a RomFS entry path from a borrowed str.
    ///
    /// # Safety
    ///
    /// The caller must ensure the path upholds the entry path invariants: it is
    /// non-empty, relative, `/`-separated, and contains no `..` component. This
    /// constructor performs no validation.
    pub fn from_ref_unchecked(path: &'a str) -> Self {
        Self(Cow::Borrowed(path))
    }
}
```

A type that offers an unchecked constructor also says so at the module level: a `//!` block stating which
invariants the type maintains, and where validation actually happens.

## 5. Every Call Site Carries a `// SAFETY:` Comment

The comment goes immediately above the call and names **why the invariant already holds** here. "This is
fine", "we know it's valid", and a restatement of the function's own docs are all failures — they record that
someone thought about it, not what they concluded. Without one, a later change to the invariant has no
searchable list of sites to re-examine, and the invalid value enters the domain from the one path nobody
audits.

```rust
// ✅ Good — the comment names the reason the proof exists, so a reviewer can check the claim
// rather than the code.
impl<'a> RomFsDir<'a> {
    pub fn entries(&self) -> impl Iterator<Item = EntryPath<'a>> + '_ {
        self.names().map(|name| {
            // SAFETY: `RomFsBuilder` rejects absolute, empty, and `..`-bearing paths before
            // writing the name table, so every name read back upholds the invariants.
            EntryPath::from_ref_unchecked(name)
        })
    }
}
```

**Test code is the one exception.** A call in a `#[cfg(test)]` module needs no `// SAFETY:` comment: the value
is a literal in the same expression, visible to anyone reading the assertion, and the comment would bury the
thing the test is actually about.

`From`/`Into` impls are the other common site, and the rule does not soften there: a conversion that re-wraps
an already-valid value writes `// SAFETY: The input already upholds the invariants` rather than relying on the
reader to reconstruct it.

## 6. Where Unchecked Construction Is Never Acceptable

Anything that arrives from outside is parsed, never wrapped: a CLI argument, a project manifest, an
environment variable, a path walked off the filesystem, bytes read from an NRO someone else built. Wrapping
one asserts the fact the boundary exists to establish, in a place with no error path — so the invalid value
surfaces later, mid-build, with nothing tying it to the input that introduced it
([principle-validate-at-edge](principle-validate-at-edge.md)).

```rust
// ❌ Bad — a CLI argument wrapped, not parsed. A `../..` path now flows into the builder,
// and the first sign of it is a RomFS image whose entries escape the bundle root.
let path = EntryPath::from_owned_unchecked(args.romfs_entry);
```

```rust
// ✅ Good — the boundary parses, and the caller gets a typed error to report.
let path = args.romfs_entry.parse::<EntryPathOwned>().map_err(Error::from)?;
```

## Checklist

Before committing code, verify:

- [ ] The type's validating constructor is `FromStr` or `TryFrom`, and it is the only place the invariant is
      checked
- [ ] Every constructor that skips validation has `_unchecked` in its name
- [ ] The declaration carries a `# Safety` doc section naming the invariant the caller must uphold
- [ ] The constructor's visibility is as narrow as its callers allow
- [ ] Every call site outside `#[cfg(test)]` has a `// SAFETY:` comment immediately above it, naming why the
      invariant already holds
- [ ] No `// SAFETY:` comment merely restates the function's docs or asserts that the value is valid
- [ ] No value from a CLI argument, a manifest, a filesystem path, or a foreign image is wrapped rather than parsed
- [ ] The module documents which invariants the type maintains and where validation actually occurs

## References

- [principle-validate-at-edge](principle-validate-at-edge.md) - Foundation: Where the invariant is established,
  and why a trusted store is a different case from an untrusted request
- [principle-type-driven-design](principle-type-driven-design.md) - Foundation: The newtype carries the proof
  that an unchecked constructor asserts
- [principle-least-surprise](principle-least-surprise.md) - Foundation: `FromStr`/`TryFrom` are the constructors a
  reader expects; a bypass must announce itself in its name
- [rust-docs-rustdoc](rust-docs-rustdoc.md) - Related: The rustdoc sections a `# Safety` block sits among, and
  module-level invariant docs
