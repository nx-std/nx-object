---
name: "principle-dry-wet"
description: "DRY vs WET — deduplicate knowledge, tolerate coincidental similarity. Load when extracting shared helpers, creating abstractions, or reviewing duplicated-looking code"
type: "principle"
scope: "global"
---

# DRY/WET Balance (Don't Repeat Yourself vs. Write Everything Twice)

**MANDATORY for ALL code in the workspace**

## Rule

Every piece of **knowledge** — a formula, a wire format, an on-disk layout, a policy — has exactly one
authoritative representation. Deduplicate knowledge. Do **not** deduplicate code that merely looks alike but
belongs to independent concerns that will diverge.

Before extracting a shared helper, apply these checks:

1. **Same knowledge, not same shape**: does the duplication encode the same fact? An artifact's location under
   `target/` is one fact about the output layout. Two packing loops targeting _different container formats_
   are two facts that happen to look alike.
2. **Rule of Three**: resist extracting on the second occurrence. Wait for the third, when you can see which
   parts actually vary.
3. **Inline test for a wrong abstraction**: if the shared function has a parameter or conditional whose only
   job is to pick a caller's behavior, the abstraction is wrong. Inline it back and let each caller evolve.

**Duplication is far cheaper than the wrong abstraction.** Inlining a premature abstraction is progress.

## Examples

1. **Same knowledge — one authoritative representation**
   Where a built artifact lands under `target/` is a fact about the output layout; the build command, the
   bundler, and the status line all need it.

```rust
// ❌ Bad — the output-path convention is re-derived in three places. Adding the target
// triple to the path means finding every format string, and missing one leaves the
// bundler picking up a stale NRO from a path the build no longer writes to.
let out = format!("target/{profile}/{name}.nro");
// ...and again where the bundler picks up the NRO to wrap into an NSP
let input = PathBuf::from(format!("target/{}/{}.nro", spec.profile, spec.name));
// ...and again where the command reports where the artifact went
ui::status(format!("wrote target/{profile}/{name}.nro"));
```

```rust
// ✅ Good — one module owns the fact; everything else asks for it. Adding the target
// triple becomes a one-line change instead of a search.
/// Path a built artifact is written to under `target/`.
pub fn artifact_path(spec: &BuildSpec, format: OutputFormat) -> PathBuf {
    PathBuf::from(format!("target/{}/{}.{}", spec.profile, spec.name, format.extension()))
}
```

2. **Coincidental similarity — keep them separate**
   Two sets of file names, structurally identical, encoding different knowledge.

```rust
// ❌ Bad — one shared set, because "they're both lists of special file names".
// Now excluding a new host build file from the packed image silently makes it
// un-declarable as a RomFS entry, and reserving a name silently drops it from bundles.
pub const SPECIAL_ENTRIES: &[&str] = &["icon.jpg", "control.nacp", ".gitignore", "Cargo.toml"];
```

```rust
// ✅ Good — two facts, two homes, free to diverge. Neither set can be edited
// into changing the other's behavior, because neither set is the other's.
/// A FORMAT fact: names the NRO asset section owns; a RomFS entry may not use them.
const RESERVED_ENTRIES: &[&str] = &["icon.jpg", "control.nacp"];

/// A BUNDLING policy: host build files omitted from the packed image.
const EXCLUDED_ENTRIES: &[&str] = &[".gitignore", "Cargo.toml"];
```

3. **Wrong abstraction — inline it back**
   One packer turns segments into an **NRO**, another into an **NSP**. They look almost identical. They are
   not the same knowledge.

```rust
// ❌ Bad — one "universal" packer with a format flag. The two paths already differ (an NSP
// carries an NPDM and a PFS0 index; an NRO carries an asset section, and must emit no
// asset section at all when there are no assets rather than an empty one). Every format
// change adds another flag, and each flag is a chance to break the other caller.
fn pack(
    segments: &[Segment],
    format: OutputFormat,
    options: PackOptions, // { write_npdm, embed_assets, empty_as_none }
) -> Result<Vec<u8>, PackError> {
    // ~80 lines of `if format == OutputFormat::Nsp { ... } else { ... }`
}
```

```rust
// ✅ Good — two packers, each owning one container format's rules, each independently
// testable. A change to NSP layout cannot reach the NRO packer's tests.
/// Pack segments into an NRO. No assets produces no asset section.
pub fn pack_nro(segments: &[Segment], assets: Option<&Assets>) -> Result<Vec<u8>, PackError> {
    // ...
}

/// Pack segments into an NSP, carrying the NPDM descriptor and the PFS0 index the
/// container requires.
pub fn pack_nsp(segments: &[Segment], npdm: &NpdmSpec) -> Result<Vec<u8>, PackError> {
    // ...
}
```

## Why It Matters

Duplicated knowledge means coordinated edits. Miss one copy of an output-path template and the build writes to
a path nothing reads, while the deploy uploads yesterday's binary — no compile error, no test failure, and a
console running code nobody changed.

Duplicated _shape_ forced into one abstraction costs more. A shared `pack(segments, format, options)` couples
one container format to the other: a change on one side touches code the other side's tests cover, and the
flags grow until nobody can say what the function does without reading every branch. Undoing that is harder
than never building it.

## Pragmatism Caveat

The Rule of Three is a heuristic. Two occurrences of an unmistakable fact (a format's magic bytes, a protocol
constant like `MAX_FILE_CHUNK_SIZE`) can be extracted immediately; three occurrences that serve three
container formats should stay apart.

Small helpers duplicated across module or crate boundaries are usually correct. A four-line conversion helper
copied into two sibling modules is not a violation: promoting it to `pub(crate)` or to a shared crate to save
eight lines widens an API surface and stops the two modules changing independently. Prefer the private copy.

When you keep duplication on purpose, say so in a comment. When you extract, make sure the name describes the
shared _concept_ (`artifact_path`), not the shared _shape_ (`pack`, `handle_thing`). An undocumented decision
either way is always wrong.

## Checklist

Before committing code, verify:

- [ ] Extracted code encodes one fact, not one syntax shape
- [ ] No shared helper takes a flag, mode, or `kind` parameter that exists only to select a caller's behavior
- [ ] Wire-format constants, on-disk layouts, and spec values have exactly one definition
- [ ] Similar-looking code that serves two container formats or two policies stays in two places
- [ ] Deliberate duplication carries a comment explaining that the similarity is coincidental
- [ ] Cross-crate hoisting is justified by shared knowledge, not by line count

## References

- [principle-single-responsibility](principle-single-responsibility.md) - Related: A helper serving two concerns
  is the wrong abstraction by definition
- [principle-open-closed](principle-open-closed.md) - Related: Registries and extension points are where
  genuinely shared behavior belongs; flags are not
- [principle-least-surprise](principle-least-surprise.md) - Related: An abstraction named for its shape rather
  than its concept surprises every caller
- [principle-symmetry](principle-symmetry.md) - Related: Make near-duplicates symmetric first; only then is it
  visible whether they are one fact or two
- [principle-rate-of-change](principle-rate-of-change.md) - Related: Two copies that change on different
  schedules are two facts, whatever their shape says

## External References

- [The Wrong Abstraction — Sandi Metz](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction)
- [DRY is about Knowledge (Verraes)](https://verraes.net/2014/08/dry-is-about-knowledge/)
- [Caught in a Bad Abstraction — Israeli Tech Radar](https://medium.com/israeli-tech-radar/caught-in-a-bad-abstraction-55bfe6634b83)
- [DRY: Most Over-rated Programming Principle — Gordon C](https://gordonc.bearblog.dev/dry-most-over-rated-programming-principle/)
