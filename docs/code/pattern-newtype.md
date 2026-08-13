---
name: "pattern-newtype"
description: "Newtypes for identity, invariants, and units: private field, FromStr as the only validator. Load when a domain value is a bare String or integer, or two same-typed parameters sit side by side"
type: "core"
scope: "global"
---

# Newtype (Wrapped Primitive)

**MANDATORY for ALL Rust code in the workspace**

## Rule

A domain value carried as a bare `String` or integer makes every reader remember an invariant the compiler
could have remembered for them. When the value has an **identity**, an **invariant**, or a **unit**, declare it
as a newtype — `struct EntryPath(String)` — a primitive the compiler refuses to interchange with its
neighbours.

A value earns a newtype when it does at least one of three jobs:

1. **Identity** — same-typed values that must never be swapped: the path a file has on the host and the path
   it takes inside the image, meeting in one function.
2. **Invariant** — a constraint established once, at the edge, and never re-checked: a non-zero chunk size, a
   relative `/`-separated entry path.
3. **Unit** — a unit, base, or convention a bare primitive cannot state: a file offset versus a memory offset,
   inclusive end versus exclusive end, bytes versus pages.

Three signals a newtype is missing, all visible in a diff:

- Two parameters of the same primitive type sit side by side (`add(src: &str, dst: &str)`).
- A `- 1` or `+ 1` whose meaning lives in a comment rather than in a type.
- A doc comment saying what the type should have said: _"must be relative"_, _"exclusive"_, _"in pages"_.

**Declaring.** A tuple struct with a **private** field, so construction cannot bypass the invariant. The
validating constructor is `FromStr` (or `TryFrom`) and nothing else — see
[rust-fn-unchecked](rust-fn-unchecked.md) for the narrow cases where a constructor may skip it. Add `Display`,
`AsRef<str>`, and `#[serde(try_from = "...")]`, so the type is as convenient as the primitive it replaces and
deserialization runs the same check.

**Constructing.** At the boundary the value enters through, and nowhere else. A newtype constructed all over
the domain is a newtype whose invariant nobody can locate.

## Examples

1. **Two paths that are not the same path**
   Adding an asset takes the file's path on the host and the path it will have inside the image. Both are
   strings, they sit side by side, and transposing them produces a plausible-looking call that compiles.

```rust
// ❌ Bad — two bare strings in the same signature. `add_asset(entry, source)` compiles,
// stores the file under a host-absolute path inside the image, and the only symptom is
// an application that cannot open a file the bundle appears to contain.
pub fn add_asset(source: &str, entry: &str, contents: &[u8]) -> Result<(), AddError> {}
```

```rust
// ✅ Good — two types, so the transposition is a compile error at every call site, forever.
pub struct SourcePath(PathBuf);
pub struct EntryPath(String);

pub fn add_asset(
    source: &SourcePath,
    entry: &EntryPath,
    contents: &[u8],
) -> Result<(), AddError> {}
```

2. **A base written into the type, and enforced at the edge**
   Segment bounds are half-open everywhere in the packer, but `u32` cannot say so, and a caller that reads one
   as inclusive copies a byte past the end of every segment.

```rust
// ❌ Bad — the convention lives in a doc comment, and the `+ 1` that reconciles the two
// readings is copied to three call sites. The one that dropped it copies one byte too
// few, so the last instruction of `.text` never reaches the image and the executable
// faults at a different address every build.
/// Copy bytes `start` through `end`, inclusive.
pub fn copy_segment(start: u32, end: u32, out: &mut Vec<u8>) -> Result<(), CopyError> {}

let copied = copy_segment(bounds.offset, bounds.end - 1, &mut out)?;
```

```rust
// ✅ Good — the convention is the type, and the one conversion between the two lives in
// one function. A missing conversion is now a missing call, not an absent `+ 1`.
pub struct FileOffset(u32);

/// Half-open segment bounds: `offset` included, `end` excluded.
pub struct SegmentBounds { offset: FileOffset, end: FileOffset }

impl SegmentBounds {
    /// The last byte in the segment, for headers that record ends inclusively.
    pub fn last(&self) -> FileOffset {
        FileOffset(self.end.0 - 1)
    }
}

pub fn copy_segment(bounds: &SegmentBounds, out: &mut Vec<u8>) -> Result<(), CopyError> {}
```

3. **Brand the subset a function actually requires**
   A console address is a socket address, but not every socket address is one the loader can reach. When a
   _subset_ of a type is what a function requires, make the subset a type too, narrowed once where the value
   is parsed rather than re-checked defensively at each call.

```rust
// ❌ Bad — every caller is trusted to have checked the family, and the one that did not
// hands an IPv6 address straight to the transfer, which fails at connect time with an
// error naming neither the flag it came from nor the value.
pub async fn send_nro(console: &SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {
    TcpStream::connect(console).await
}
```

```rust
// ✅ Good — two types, because there are two claims: it is a socket address, and it is
// one the netloader protocol can use. The narrowing happens once, where it is parsed.
pub struct ConsoleAddr(SocketAddrV4);

impl TryFrom<SocketAddr> for ConsoleAddr {
    type Error = NotAConsoleAddr;

    fn try_from(addr: SocketAddr) -> Result<Self, Self::Error> {
        match addr {
            SocketAddr::V4(addr) => Ok(Self(addr)),
            SocketAddr::V6(_) => Err(NotAConsoleAddr::Ipv6),
        }
    }
}

// Cannot be called with an IPv6 address, so it needs no check and has no failure mode.
pub async fn send_nro(console: &ConsoleAddr, nro: &[u8]) -> Result<(), SendNroError> {}
```

## Why It Matters

**The compiler remembers, so the reader does not.** "The first argument is the source path"; "these bounds are
half-open"; "this string is an entry path, not a host path" — each is a fact a maintainer holds in their head
at every call site, and forgets exactly once.

**API misuse becomes a compile error instead of a plausible result.** The failures a newtype prevents are the
quiet ones: a transposed path writes a well-formed entry in the wrong place; a dropped `- 1` truncates every
segment by a byte. Neither panics, neither fails a test, and both are ruled out once the types differ.

**An invariant checked at the edge stays checked.** A `ChunkSize` cannot exist without being non-zero, so
nothing downstream re-validates it or has to trust a comment — that is
[principle-validate-at-edge](principle-validate-at-edge.md) in the type system rather than in discipline.

**It costs nothing at runtime**: a tuple struct has the primitive's layout, and only the validating constructor
runs a check, once, at the boundary. **The documentation cannot go stale**, because it is the signature.

## Pragmatism Caveat

Wrap a value that has an invariant, a unit, or a confusable sibling; a newtype with nothing to distinguish is ceremony:

- **No sibling, no invariant, no newtype.** A free-form description, a log message, an operator-supplied label:
  nothing to swap them with, nothing to check. Leave them `String`.
- **Named struct fields blunt the swap hazard** positional parameters create. Two values that are genuinely the
  same kind playing different roles want a struct with named fields, not two newtypes differing in name only.
- **A newtype is not a validator for external state.** "Does this file exist?", "is this console still
  listening?" depend on the world, not the value. Those stay runtime checks in the domain.
- **Keep validation off hot paths.** The check runs on every construction: right at a boundary, wrong per
  chunk.
- **If a newtype forces `*_unchecked` calls at ordinary call sites, the boundary is in the wrong place.** Move
  construction to the edge the value actually enters through; see [rust-fn-unchecked](rust-fn-unchecked.md).

## Checklist

Before committing code, verify:

- [ ] Every domain value with an invariant, a unit or base, or a confusable same-typed sibling is a newtype
- [ ] The wrapped field is private, so construction cannot bypass the invariant
- [ ] The validating constructor is `FromStr` or `TryFrom`; there is no second, parallel validator
- [ ] Deserialization runs the same check (`#[serde(try_from = "...")]`), rather than trusting the wire form
- [ ] The newtype is constructed at the boundary the value enters through, and nowhere else
- [ ] A conversion between two newtypes (half-open to inclusive, name to prefix) exists in exactly one function
- [ ] `Display`, `AsRef`, and the primitive's other conveniences are provided, so callers never reach for the inner value
- [ ] No newtype was added to a value with nothing to confuse it with and no invariant to carry

## References

- [principle-type-driven-design](principle-type-driven-design.md) - Foundation: A newtype makes an invalid value unrepresentable
- [principle-validate-at-edge](principle-validate-at-edge.md) - Foundation: Constructed at the edge, never re-checked downstream
- [principle-least-surprise](principle-least-surprise.md) - Foundation: A signature saying `&str` twice surprises the caller who transposes them
- [rust-fn-unchecked](rust-fn-unchecked.md) - Related: The narrow cases where construction may skip the validating constructor, and the comment that must accompany it
- [pattern-builder](pattern-builder.md) - Related: Construction with multiple required fields

## External References

- [Rust API Guidelines — Newtypes](https://rust-lang.github.io/api-guidelines/type-safety.html#newtypes-provide-static-distinctions-c-newtype)
- [Parse, Don't Validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
- [The Ultimate Guide to Rust Newtypes](https://www.howtocodeit.com/guides/ultimate-guide-rust-newtypes)
