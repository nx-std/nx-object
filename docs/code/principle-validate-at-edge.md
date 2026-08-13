---
name: "principle-validate-at-edge"
description: "Validate at the Edge (hard shell, soft core) — parse untrusted input once at the boundary. Load when designing CLI commands, parsing manifests or arguments, or decoding an executable image"
type: "principle"
scope: "global"
---

# Validate at the Edge (Hard Shell, Soft Core)

**MANDATORY for ALL code in the workspace**

## Rule

Every value that enters from outside — a CLI argument, a project manifest, an NPDM descriptor, an environment
variable, a linked ELF, an NRO built by someone else, bytes arriving over the netloader socket — arrives
untrusted and typed wider than reality. Parse it **once**, at the boundary, into a type the rest of the code can trust. Past that point, no
function re-checks. The boundary is the hard shell; the domain is the soft core. Concretely:

- Parsing lives in `FromStr` or `TryFrom`, not in a standalone `parse` function and not in the handler body.
  That is the one place a newtype's invariant is established, and it is what clap's `value_parser`, serde's
  `try_from`, and `?` all compose with.
- A command's job is to turn arguments and manifests into domain types and hand them on. Domain functions
  take the parsed types, never the raw argument.
- Malformed input degrades into a typed error at the edge — a rejected argument, a refused deploy, a skipped
  asset with a reported reason — never a panic three layers down in the packer.
- Unit and convention conversions happen once, at the boundary: hex string to bytes, exclusive end to
  inclusive, seconds to `Duration`.

## Examples

1. **Parse into domain types at the boundary; the domain trusts them**
   A command receives strings. Everything past it receives meaning.

```rust
// ❌ Bad — the domain function takes raw strings and validates them itself. Every other
// caller must remember to do the same, and the one that forgot packed an image whose
// entries all sat under an empty path, because an empty string parsed to a default.
fn add_asset(entry: &str, source: &str, out: &mut RomFsBuilder) -> Result<(), AddError> {
    if entry.is_empty() {
        return Err(AddError::MissingEntryPath);
    }
    if entry.starts_with('/') {
        return Err(AddError::AbsoluteEntryPath);
    }
    let source = PathBuf::from(source);
    if !source.is_file() {
        return Err(AddError::NotAFile);
    }
    // ...domain logic, finally
}
```

```rust
// ✅ Good — the invariants live in FromStr; the command parses; the domain trusts.
// Every path into the domain goes through the same types, including the bundle command.
impl std::str::FromStr for EntryPath {
    type Err = ParseEntryPathError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(ParseEntryPathError::Empty);
        }
        if input.starts_with('/') {
            return Err(ParseEntryPathError::Absolute);
        }
        if input.split('/').any(|component| component == "..") {
            return Err(ParseEntryPathError::ParentComponent);
        }
        Ok(Self(input.to_owned()))
    }
}

// Boundary
fn run_add(args: AddArgs, out: &mut RomFsBuilder) -> Result<(), Error> {
    let entry = args.entry.parse().map_err(Error::from)?;
    let source = SourcePath::try_from(args.source).map_err(Error::from)?;
    add_asset(entry, source, out).map_err(Error::from)
}

// Domain — no defensive checks; the types carry the proof
fn add_asset(entry: EntryPath, source: SourcePath, out: &mut RomFsBuilder) -> Result<(), AddError> {}
```

2. **Cross-field constraints belong to a composite type**
   A relationship between two fields is an invariant of the pair, so the pair is the type.

```rust
// ❌ Bad — the ordering check sits in the one function that happened to need it,
// and the exclusive/inclusive convention is re-decided at every call site.
fn copy_segment(offset: FileOffset, end: FileOffset, out: &mut Vec<u8>) -> Result<(), CopyError> {
    if offset > end {
        return Err(CopyError::InvertedBounds);
    }
    // is `end` inclusive here? the caller two modules up assumed it was not
}
```

```rust
// ✅ Good — the pair is a type, the ordering is its invariant, and the convention is
// stated once and carried in the name.
/// Half-open segment bounds: `offset` included, `end` excluded.
pub struct SegmentBounds {
    offset: FileOffset,
    end: FileOffset,
}

impl TryFrom<(u32, u32)> for SegmentBounds {
    type Error = ParseSegmentBoundsError;

    fn try_from((offset, end): (u32, u32)) -> Result<Self, Self::Error> {
        if offset >= end {
            return Err(ParseSegmentBoundsError::Empty { offset, end });
        }
        Ok(Self { offset: FileOffset(offset), end: FileOffset(end) })
    }
}

fn copy_segment(bounds: SegmentBounds, out: &mut Vec<u8>) -> Result<(), CopyError> {}
```

3. **Configuration is deserialized into validated types, once**
   A config value checked wherever it is consumed is a config value that is eventually consumed somewhere new.

```rust
// ❌ Bad — the raw descriptor is trusted, and every consumer re-checks the parts it uses.
// The consumer added last did not, and a zero main-thread stack size produced an NPDM
// the loader rejected, with the failure reported by the console rather than the build.
#[derive(Deserialize)]
pub struct NpdmSpec {
    pub title_id: String,
    pub main_thread_stack_size: u32,
}

pub fn build_npdm(spec: &NpdmSpec) -> Result<Vec<u8>, BuildError> {
    if spec.title_id.is_empty() {
        return Err(BuildError::MissingTitleId);
    }
    if spec.main_thread_stack_size == 0 {
        return Err(BuildError::BadStackSize);
    }
    encode_npdm(&spec.title_id, spec.main_thread_stack_size)
}
```

```rust
// ✅ Good — deserialization is the boundary. An invalid descriptor fails at load, naming
// the field and the reason, before a single byte is packed.
#[derive(Deserialize)]
pub struct NpdmSpec {
    pub title_id: TitleId,
    pub main_thread_stack_size: StackSize,
}

#[derive(Deserialize)]
#[serde(try_from = "u32")]
pub struct StackSize(NonZeroU32);

impl TryFrom<u32> for StackSize {
    type Error = ParseStackSizeError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        NonZeroU32::new(value).map(Self).ok_or(ParseStackSizeError::Zero)
    }
}

pub fn build_npdm(spec: &NpdmSpec) -> Result<Vec<u8>, BuildError> {
    encode_npdm(spec.title_id, spec.main_thread_stack_size.get())
}
```

4. **A malformed external image degrades; it does not take the process down**
   An NRO handed to the tool was built by someone else. It will eventually contain something the format's
   documentation does not describe.

```rust
// ❌ Bad — the image is trusted to have the shape the header claims. A segment whose
// offset plus size runs past the end of the buffer panics on the slice, so the tool
// aborts with an index-out-of-bounds message naming neither the file nor the segment.
fn segment_bytes(image: &[u8], header: &NroHeader, index: usize) -> &[u8] {
    let segment = &header.segments[index];
    let offset = segment.file_off.get() as usize;
    &image[offset..offset + segment.size.get() as usize]
}
```

```rust
// ✅ Good — decoding is fallible at the edge, and the failure carries what is needed
// to act on it: which segment, what bound it broke, how large the image actually is.
fn segment_bytes(image: &[u8], header: &NroHeader, index: usize) -> Result<&[u8], SegmentError> {
    let segment = header
        .segments
        .get(index)
        .ok_or(SegmentError::NoSuchSegment { index })?;
    let offset = segment.file_off.get() as usize;
    let end = offset
        .checked_add(segment.size.get() as usize)
        .ok_or(SegmentError::BoundsOverflow { index })?;
    image
        .get(offset..end)
        .ok_or(SegmentError::OutOfBounds { index, end, available: image.len() })
}
```

## Why It Matters

These tools sit between user-authored manifests, a compiler's output, and binaries built by other toolchains,
any of which can produce a value the types do not describe: a segment that runs past the end of its file, an
entry path that escapes the bundle root, a stack size of zero. Trusted where they land, the failure surfaces
somewhere else entirely — a panic in the packer caused by an argument accepted at startup, or worse, a
well-formed artifact that only fails on the console.

A single narrow point also localizes the fix. When a format's field changes shape, the one decode site is what
changes, and a failed decode is a reported, skipped input rather than an aborted build. Because the invariant
lives in `FromStr`, every entry point that reaches the same domain — the `build` command, the `bundle`
command, the library API — enforces it identically, for free. Scattered per-layer checks buy the opposite:
three partial contracts, and the union of them is nobody's job.

## Pragmatism Caveat

The signal for where a check belongs is what it depends on:

- **Depends only on the incoming value → the edge**: shape, required fields, formats, ranges, and cross-field
  constraints within one payload.
- **Depends on external state → the domain**: "does this source file still exist?", "is the console still
  listening?", "is there space on the device?". These need the world, and the edge does not have it.

Do not push state-dependent checks into the boundary, and do not let shape checks leak past it. Do not
re-validate in the soft core: a function taking a parsed type does not check its fields again. An undocumented
re-validation is dead code that hides where the real contract lives.

## Checklist

Before committing code, verify:

- [ ] Every invariant is established in `FromStr` or `TryFrom`, not in a handler body or a standalone `parse`
- [ ] Handlers parse the request into domain types; domain functions take the parsed types, never raw input
- [ ] Cross-field constraints are invariants of a composite type, not checks in one function that needed them
- [ ] Manifests and CLI input are deserialized into validated types at load; consumers do not re-check
- [ ] Foreign images are decoded fallibly, with errors naming the field and the context, never `unwrap`
- [ ] Downstream functions contain zero re-validation of what the boundary guaranteed
- [ ] Unit and convention conversions (hex to bytes, exclusive to inclusive) happen once, at the boundary
- [ ] Checks that require external state stay in the domain

## References

- [principle-type-driven-design](principle-type-driven-design.md) - Related: The edge produces the validated
  types that make illegal states unrepresentable
- [principle-idempotency](principle-idempotency.md) - Related: The boundary that accepts an input is where the
  identity of the work it causes is established
- [principle-least-surprise](principle-least-surprise.md) - Related: A function whose parameter is a parsed type
  must behave as though it trusts it

## External References

- [Parse, Don't Validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
- [Using Types To Guarantee Domain Invariants](https://lpalmieri.com/posts/2020-12-11-zero-to-production-6-domain-modelling/)
- [Architecture Patterns with Python (O'Reilly)](https://www.oreilly.com/library/view/architecture-patterns-with/9781492052197/)
