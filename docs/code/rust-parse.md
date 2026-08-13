---
name: "rust-parse"
description: "FromStr fully qualified and never imported, .parse() at call sites, annotate only when inference needs it. Load when implementing FromStr or parsing a string"
type: "core"
scope: "global"
---

# Parsing With `FromStr`

**MANDATORY for ALL Rust code in the workspace**

That a parseable type implements `FromStr` at all is owned by
[principle-least-surprise](principle-least-surprise.md); that the invariant it checks lives there and nowhere
else is owned by [principle-validate-at-edge](principle-validate-at-edge.md). This document is about how the
impl and its call sites are written.

## 1. `FromStr` Is Implemented Fully Qualified

Write `impl std::str::FromStr for T`. **Never import the trait.** An import earns its place by shortening
something that appears repeatedly; `FromStr` appears exactly once per type, in the `impl` header, and the
qualified path says which trait it is without the reader scrolling to the prologue. A bare `impl FromStr for T`
could name any of several traits called `FromStr` in the dependency graph.

```rust
// ✅ Good — no import, and the header names the trait unambiguously.
impl std::str::FromStr for TitleId {
    type Err = ParseTitleIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {}
}
```

The associated error type is `Err`, not `Error`, and it is a named error type — see
[rust-errors-reporting](rust-errors-reporting.md).

## 2. Call Sites Use `.parse()`

`str::parse` is an inherent method bounded by `FromStr`, so the trait does **not** need to be in scope to call
it. That is why the import in §1 is unnecessary: nothing outside the `impl` ever names the trait.

Never call `T::from_str(s)` or `FromStr::from_str(s)` directly. They are the same function reached by a longer
path, and `T::from_str` additionally requires the trait in scope in some positions, which reintroduces the
import that should not exist.

```rust
// ❌ Bad — the trait path at the call site, and an import to make it resolve.
use std::str::FromStr;

let title_id = TitleId::from_str(&args.title_id)?;
let port = u16::from_str(&raw)?;
```

```rust
// ✅ Good — the inherent method; no import, no trait named at the call site.
let title_id = args.title_id.parse::<TitleId>()?;
let port = raw.parse::<u16>()?;
```

These two carry a turbofish only because the snippets have nothing around them to infer from. In real code
most parse sites need no annotation at all: see [§3](#3-annotate-only-when-inference-needs-it).

## 3. Annotate Only When Inference Needs It

**Write `.parse()?` bare.** If it compiles, the compiler already knew the type, and naming it again adds a
token the reader has to check against the signature that actually decides it. Most parse sites are inferred,
because something downstream pins the type: a direct return, a struct field initializer, an argument to a
typed parameter, a comparison against a typed value, or a later use of the binding.

```rust
// ❌ Bad — every one of these types is already determined. The annotation is a
// second statement of the same fact, and it silently becomes a lie if the
// signature or field type it duplicates changes.
fn title_id(raw: &str) -> Result<TitleId, ParseTitleIdError> {
    raw.parse::<TitleId>()
}

let spec = NpdmSpec {
    title_id: raw.parse::<TitleId>()?,
    is_64_bit,
};
```

```rust
// ✅ Good — the type is stated once, by the signature or the field, and the parse
// reads as what it is: turn this string into whatever is required here.
fn title_id(raw: &str) -> Result<TitleId, ParseTitleIdError> {
    raw.parse()
}

let spec = NpdmSpec {
    title_id: raw.parse()?,
    is_64_bit,
};
```

When inference genuinely cannot resolve the target — nothing downstream constrains it, or the value is
consumed by something generic — state it with a **turbofish on the call**, never as an annotation on the
binding.

```rust
// ❌ Bad — the type is on the binding, away from the operation that produces it.
// Inline the binding, pass it as an argument, or move it into a match scrutinee,
// and the annotation has nowhere to go.
let timeout_ms: u32 = raw_timeout.parse()?;
```

```rust
// ✅ Good — the type rides with the call, so it survives being moved into any
// expression position, including one with no binding to annotate at all.
let timeout_ms = raw_timeout.parse::<u32>()?;

if raw_timeout.parse::<u32>()? > MAX_DISCOVERY_TIMEOUT_MS {
    return Err(Error::TimeoutTooLarge);
}
```

## 4. Not Every `from_str` Is `FromStr`

`serde_json::from_str` (and its siblings in other format crates) is a deserializer entry point, not the
`FromStr` method. It takes a `&str` and a target that implements `Deserialize`, and it is called by its
qualified path as usual — `serde_json::from_str::<NpdmSpec>(payload)?`. Nothing here applies to it.

## Checklist

Before committing code, verify:

- [ ] `FromStr` is implemented as `impl std::str::FromStr for T`, fully qualified
- [ ] No file imports `std::str::FromStr`
- [ ] Call sites use `.parse()`; no `T::from_str(..)` or `FromStr::from_str(..)`
- [ ] `.parse()` is written bare wherever the type is inferable; no annotation restates what a signature,
      field type, or parameter already pins
- [ ] Where inference genuinely cannot resolve the target, it is stated with a turbofish on the call, never as
      an annotation on the binding
- [ ] `serde_json::from_str` and other deserializer entry points are left alone

## References

- [principle-least-surprise](principle-least-surprise.md) - Foundation: Why a parseable type implements
  `FromStr` rather than a custom constructor
- [principle-validate-at-edge](principle-validate-at-edge.md) - Foundation: `FromStr` is where the invariant is
  established, and the only place it is checked
- [pattern-newtype](pattern-newtype.md) - Related: The types that earn a `FromStr` impl
- [rust-imports](rust-imports.md) - Related: The general rule that one-off `std` paths stay qualified
- [rust-fn-unchecked](rust-fn-unchecked.md) - Related: The narrow cases where construction may bypass
  `from_str`
- [rust-errors-reporting](rust-errors-reporting.md) - Related: Designing the `Err` type the impl returns
