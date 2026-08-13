---
name: "rust-fn-conv"
description: "Conversions: implement From, TryFrom when fallible, .into() at call sites, and when an as cast is allowed. Load when converting between types or reviewing an as cast"
type: "core"
scope: "global"
---

# Type Conversions

**MANDATORY for ALL Rust code in the workspace**

Rust offers several ways to turn one type into another, and they are not equivalent: two are checked by the
compiler, one is silent about losing data, and one is unchecked entirely. Reach for them in this order.

| Mechanism             | Use it                                              |
|-----------------------|-----------------------------------------------------|
| `From` / `Into`       | Whenever the conversion cannot fail                 |
| `TryFrom` / `TryInto` | Whenever it can                                     |
| `as`                  | Only where the bound is proven and stated in a comment |
| `transmute`           | Never                                               |

This document owns which mechanism to use. What to **name** a conversion method — `as_*`, `to_*`, `into_*` —
belongs to [rust-fn](rust-fn.md), and parsing from strings belongs to [rust-parse](rust-parse.md).

## 1. Implement `From`, Never `Into`

The standard library provides a blanket `impl<T, U: From<T>> Into<U> for T`, so writing `From` gives `Into`
for free; the reverse does not hold. An `Into` impl is strictly less useful than the `From` it should be:
nothing gains `From`, so `SegmentBounds::from(header)` is a compile error for no reason, and the impl cannot
participate where a `From` bound is required.

```rust
// ✅ Good — one impl, both directions of call site.
impl From<NroSegmentHeader> for SegmentBounds {
    fn from(header: NroSegmentHeader) -> Self {}
}
```

Trait **bounds** invert this: accept `impl Into<T>` — `pub fn title(name: impl Into<AppTitle>)` — rather
than `T: From<..>`, because an `Into` bound is satisfied by every `From` impl plus any direct `Into` a foreign
crate may have written.

## 2. Call `.into()`, Not `T::from()`

At a call site the target type is almost always already known — from the parameter, the field, or the return
type — so `.into()` reads in the order the data flows and needs no repetition of the type.

```rust
// ❌ Bad — names the destination type again, in the middle of an expression that
// already establishes it, and reads inside-out.
let title = AppTitle::from(raw);
pack(NroSpec::from(manifest), assets);
```

```rust
// ✅ Good — the value first, the conversion second, the type left to the context.
let title = raw.into();
pack(manifest.into(), assets);
```

`T::from(x)` stays correct where inference genuinely has nothing to work with, and in a `map` where the
function reference is the point: `headers.map(SegmentBounds::from)` beats `headers.map(|h| h.into())`.

## 3. `TryFrom` Carries the Failure

A conversion that can fail is a `TryFrom` impl with a named error type, not a free function returning `Option`,
and not a `From` impl that asserts its input — a `From` that panics lies about totality, and every caller
inherits a panic they cannot see in the signature.

```rust
// ✅ Good — the failure is in the type, with an error the caller can report.
impl TryFrom<u32> for ChunkSize {
    type Error = ParseChunkSizeError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        NonZeroU32::new(value).map(Self).ok_or(ParseChunkSizeError::Zero)
    }
}
```

Implementing `TryFrom` also gives `TryInto`, and both compose with `?`.

## 4. `as` Is Silent About What It Loses

`as` converts **without telling anyone what it discarded**. It is not a C-style "reinterpret if it fits": it
always compiles and always produces a value, whatever the input.

| Conversion                     | What `as` does                                 |
|--------------------------------|------------------------------------------------|
| Wider integer to narrower      | Truncates, keeping the low bits                |
| Signed to unsigned (same size) | Reinterprets the bit pattern                   |
| Float to integer               | Saturates at the bounds; `NaN` becomes `0`     |
| Integer to float               | Rounds, losing precision past the mantissa     |

Every one of those is a bug that ships silently. **Use `try_into()` and handle the failure.**

```rust
// ❌ Bad — compiles, runs, and writes a length four gigabytes short the day someone
// sends a larger NRO. The console reads the truncated count and stops early, and
// nothing on either side reports a failure.
let length = file_length as u32;
```

```rust
// ✅ Good — the impossible case is a typed error at the point it becomes possible.
let length =
    u32::try_from(file_length).map_err(|_| SendNroError::FileTooLarge { length: file_length })?;
```

An `as` cast is acceptable only where the bound is **structurally guaranteed** and the comment says why —
governed by [rust-docs-comments](rust-docs-comments.md). "It won't be that big" is not a bound; "the buffer
caps it at `MAX_FILE_CHUNK_SIZE`" is.

```rust
// ✅ Good — a proven bound, stated where a reader can check the claim.
// `read_len` is bounded by the buffer, which is MAX_FILE_CHUNK_SIZE (0x4000) bytes,
// so this cannot truncate.
let chunk_len = read_len as u32;
```

`transmute` is never used. A reinterpretation the type system cannot check is a correctness problem wearing a
performance argument, and nothing in this workspace needs it.

## 5. Borrowed Conversions Are `AsRef`, Not `Deref`

To hand out a borrowed view of an inner value, implement `AsRef<T>` — or an inherent `as_*` method, whose
naming [rust-fn](rust-fn.md) governs.

`Deref` is heavier: it does not add one conversion, it makes **every** method of the target type appear on the
wrapper. Deref to an immutable borrowed type (`str`, `[u8]`, `Path`) is acceptable, since everything it
exposes is read-only and the newtype's invariant survives. Deref to an owned or mutable target is always
wrong: `Deref<Target = String>` on `AppTitle` offers `push_str`, `clear`, and `insert`, so the
fits-the-NACP-field invariant the newtype exists to hold can be edited away by any caller holding a `&mut`.

```rust
// ✅ Good — an explicit borrowed view, and nothing beyond it.
impl AsRef<str> for AppTitle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// 🔶 Acceptable — Deref to the immutable borrowed form: only read-only methods.
impl std::ops::Deref for AppTitle {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}
```

Accept `impl AsRef<Path>` or `impl AsRef<str>` in a signature when a function only needs to read the value;
that admits `String`, `&str`, and every newtype above without forcing an allocation at the call site.

## Checklist

Before committing code, verify:

- [ ] Conversions are written as `From` impls; no `Into` impl was written by hand
- [ ] Trait bounds accept `impl Into<T>` rather than requiring `From`
- [ ] Call sites use `.into()` / `.try_into()`; `T::from` appears only where inference has nothing to work
      with, or as a function reference in a `map`
- [ ] Fallible conversions are `TryFrom` with a named error; no `From` impl can panic
- [ ] Numeric narrowing uses `try_into()` and handles the failure
- [ ] Every remaining `as` cast has a structurally guaranteed bound, stated in a comment at the cast
- [ ] No `transmute`
- [ ] Borrowed views are `AsRef` or an inherent `as_*` method
- [ ] Any `Deref` impl targets an immutable borrowed type (`str`, `[u8]`, `Path`), never an owned or mutable one

## References

- [rust-fn](rust-fn.md) - Extends: What a conversion method is named, and what its receiver promises
- [rust-parse](rust-parse.md) - Related: `FromStr`, the conversion that turns a string into a domain type
- [pattern-newtype](pattern-newtype.md) - Related: The wrappers most of these conversions exist for
- [rust-docs-comments](rust-docs-comments.md) - Related: Owns the comment an `as` cast must carry
- [principle-type-driven-design](principle-type-driven-design.md) - Foundation: A conversion that cannot fail
  should be unable to fail by construction, not by assertion
