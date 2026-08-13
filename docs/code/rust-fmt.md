---
name: "rust-fmt"
description: "std::fmt impls, fully qualified and never imported; Display/Debug/LowerHex/UpperHex with doctests pinning the rendering. Load when implementing Display or a hex trait"
type: "core"
scope: "global"
---

# Formatting Traits (`std::fmt`)

**MANDATORY for ALL Rust code in the workspace**

## 1. Fully Qualified, Never Imported

Every `std::fmt` item is written at its full path: the trait in the `impl` header, `std::fmt::Formatter<'_>`
in the signature, `std::fmt::Result` as the return type. **Nothing from `std::fmt` is ever imported.**

The names are the problem. `Result` and `Error` from `std::fmt` collide with the crate's own; `Write` collides
with `std::io::Write`; and a bare `Result` in a `fmt` signature reads as the crate's `Result` to everyone who
did not scroll to the prologue. Qualifying costs ten characters and removes the ambiguity permanently.

```rust
// ❌ Bad — `Result` in the signature is std::fmt's, but nothing at the use site says
// so, and the import shadows the crate's own Result for the rest of the file.
use std::fmt::{
    Display,
    Formatter,
    Result,
};

impl Display for TitleId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {}
}
```

```rust
// ✅ Good — no import, and every type in the signature names itself.
impl std::fmt::Display for TitleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {}
}
```

## 2. Delegate Through the Trait, Not Through `write!`

A newtype that renders as its inner value delegates by calling the trait function directly:

```rust
// ❌ Bad — `write!` starts a fresh format spec for the inner value, so the outer
// formatter's flags are dropped: `{:>12}` pads nothing, `{:#x}` loses its prefix.
// The bug only appears at the one call site that used a flag.
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
}
```

```rust
// ✅ Good — the same formatter is handed to the inner impl, so width, fill,
// precision, and the alternate flag all propagate.
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(&self.0, f)
}
```

Name the trait in the delegation rather than writing `self.0.fmt(f)`. When the inner type implements several
formatting traits — the normal case for a byte array or an integer — `self.0.fmt(f)` resolves by inference and
silently picks a different rendering the moment the surrounding impl changes.

## 3. The Type Documents Its Formatting Surface

A type with more than one formatting trait carries a `## Formatting` section in its own docs, stating what each
one renders and linking to the impls. A reader choosing between `{}`, `{:?}`, and `{:x}` should not have to
read three impl bodies.

```rust
/// The program title identifier recorded in an NPDM descriptor.
///
/// ## Formatting
///
/// The `TitleId` type implements the following formatting traits:
///
/// - Use [`std::fmt::Display`] for the canonical zero-padded 16-digit form.
/// - Use [`std::fmt::LowerHex`] (or [`std::fmt::UpperHex`]) for the bare hexadecimal form.
///
/// See the [`Display`], [`LowerHex`], and [`UpperHex`] trait implementations for usage examples.
///
/// [`Display`]: #impl-Display-for-TitleId
/// [`LowerHex`]: #impl-LowerHex-for-TitleId
/// [`UpperHex`]: #impl-UpperHex-for-TitleId
pub struct TitleId(u64);
```

## 4. Every `fmt` Impl Carries a Doctest

The rustdoc goes **on the `fmt` method**, not only on the type, and it states the rendering with a doctest that
asserts the exact output.

A type's rendering is a contract: it lands in logs, in CLI output, in error messages, and in serialized form
whenever `Display` backs a `serde` impl. Prose describing it drifts; an assertion does not. Without one, a
change to the encoding breaks every consumer without breaking a single test.

```rust
// ✅ Good — the doctest is the specification, and it fails the moment the rendering
// changes, whether or not anyone remembered this impl had consumers.
impl std::fmt::Display for TitleId {
    /// Format the `TitleId` in its canonical zero-padded 16-digit form.
    ///
    /// ```rust
    /// # use nx_object::TitleId;
    /// const ID: TitleId = TitleId::new(0x0100_0000_0000_1000);
    ///
    /// assert_eq!(format!("{ID}"), "0100000000001000");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}
```

## 5. Hex Traits Come in Pairs and Document the Alternate Flag

A byte-backed type that implements `LowerHex` implements `UpperHex` too. Callers pick the case at the format
site, and a type that offers only one forces the other half of the codebase into `to_uppercase()` on a
formatted string.

Both impls document that the alternate flag `#` prepends `0x`, and both doctests assert it — that behavior is
inherited from the inner type's impl, so it is easy to change by accident when the inner type is swapped.

```rust
impl std::fmt::LowerHex for TitleId {
    /// Lowercase hex representation of the `TitleId`.
    ///
    /// Note that the alternate flag, `#`, adds a `0x` in front of the output.
    ///
    /// ```rust
    /// # use nx_object::TitleId;
    /// const ID: TitleId = TitleId::new(0x0100_0000_0000_1000);
    ///
    /// assert_eq!(format!("{ID:x}"), "100000000001000");
    /// assert_eq!(format!("{ID:#x}"), "0x100000000001000");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0, f)
    }
}

impl std::fmt::UpperHex for TitleId {
    /// Uppercase hex representation of the `TitleId`.
    ///
    /// Note that the alternate flag, `#`, adds a `0x` in front of the output.
    ///
    /// ```rust
    /// # use nx_object::TitleId;
    /// const ID: TitleId = TitleId::new(0x0100_0000_0000_1000);
    ///
    /// assert_eq!(format!("{ID:X}"), "100000000001000");
    /// assert_eq!(format!("{ID:#X}"), "0x100000000001000");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.0, f)
    }
}
```

## 6. `Debug` Is Written When the Derive Is Unhelpful

`#[derive(Debug)]` is the default and stays the default. It stops being right when the derived output is
unreadable — a wrapper over a 32-byte build ID derives into a list of thirty-two integers that nobody can
match against a log line or the bytes in a hex dump.

A hand-written `Debug` picks one of two shapes and documents which:

- **Wrapped**: `BuildId(8f2c4a77...)`, keeping the type name visible in a struct dump.
- **Delegated**: whatever the inner type's `Debug` renders, when the type is meant to be indistinguishable
  from it in output.

Either way the doc on the `fmt` method says what it produces, points readers at the hex traits when they want a
specific case, and asserts the result in a doctest like the ones above.

## 7. `Display` and `FromStr` Round-Trip

When a type implements both, `value.to_string().parse::<T>()` returns the same value. A `Display` that renders
a form its own `FromStr` rejects is a defect: it breaks every log line a user tries to paste back into a
command, and it silently breaks `serde` round-trips wherever the pair backs a serialization impl.

`FromStr` may accept **more** than `Display` produces — a type that renders the zero-padded form and parses
either the padded or the bare hex form is fine, and often desirable. It may never accept less.

## Checklist

Before committing code, verify:

- [ ] Formatting traits are implemented as `impl std::fmt::<Trait> for T`, fully qualified
- [ ] No file imports anything from `std::fmt`
- [ ] `f: &mut std::fmt::Formatter<'_>` and `-> std::fmt::Result` are written at full path
- [ ] Delegation calls the trait function (`std::fmt::Display::fmt(&self.0, f)`), never `write!(f, "{}", self.0)`
      and never `self.0.fmt(f)`
- [ ] A type with more than one formatting trait has a `## Formatting` section linking to each impl
- [ ] Every `fmt` impl has rustdoc on the method with a doctest asserting the exact output
- [ ] `LowerHex` and `UpperHex` are implemented together, and both document and assert the `#` alternate flag
- [ ] A hand-written `Debug` says which shape it produces and why the derive was not used
- [ ] `Display` output parses back through `FromStr` to the same value

## References

- [rust-parse](rust-parse.md) - Related: The `FromStr` half of the round-trip, and the same fully-qualified rule
- [rust-imports](rust-imports.md) - Related: The general rule that one-off `std` paths stay qualified
- [rust-docs-rustdoc](rust-docs-rustdoc.md) - Related: The rustdoc sections and the doctest carve-out this
  document relies on
- [pattern-newtype](pattern-newtype.md) - Related: The wrappers that need a formatting surface at all
- [logging](logging.md) - Related: Where a type's `Display` and `Debug` renderings actually land
