---
name: "principle-open-closed"
description: "Open/Closed — add behaviour by adding a registry entry or a trait impl, not by editing logic that already works. Load when adding variants, extending behaviour across crates, or reviewing match chains"
type: "principle"
scope: "global"
---

# Open/Closed Principle (OCP)

**MANDATORY for ALL code in the workspace**

## Rule

Software entities should be open for extension and resistant to modification of established behavior. Add new
behavior by adding an entry to a registry, a new type implementing an existing trait, or a new value handed to
a composition root — not by editing logic that already works.

Introduce an extension point when any of these signals fires:

1. **Cross-crate extension**: behavior is added by another crate. An asset-producing crate contributes to the
   bundle without the bundling crate knowing it exists — it hands over a value implementing a trait the
   bundler declares.
2. **Externally growing variant space**: the variants track something outside the repo (container formats the
   console's loaders accept, compression schemes, target triples). New variants must be additive.
3. **Repeated branching sites**: the same `match` or `if` over the same variant set appears in more than one
   place. Consolidate it behind one lookup.

When none of these fire — the variant set is fixed by a spec, matched in one place, and local to a crate — a
plain `match` is the clearer choice.

## Examples

1. **Registry entry over a branching chain**
   Which container format gets which extension, alignment, and output path is an externally growing variant
   space. Model it as a table of specs plus one lookup, with per-variant behavior carried in the entry.

```rust
// ❌ Bad — every new output format edits three proven functions, and the three can drift
// out of sync. A format added to `extension_for` that `section_alignment_for` has never
// heard of silently takes the default alignment, and an image packed that tightly faults
// on the first `rodata` read — on hardware, where no test is looking.
pub fn extension_for(format: OutputFormat) -> Option<&'static str> {
    match format {
        OutputFormat::Nro => Some("nro"),
        OutputFormat::Nsp => Some("nsp"),
        _ => None,
    }
}

pub fn section_alignment_for(format: OutputFormat) -> u32 {
    match format {
        OutputFormat::Nro => 0x1000,
        _ => 0x200, // a new format with a different alignment means editing this too
    }
}

pub fn artifact_path(format: OutputFormat, name: &str, profile: &str) -> String {
    match format {
        OutputFormat::Nro => format!("target/{profile}/{name}.nro"),
        // ...and again here, in the one function every artifact in the workspace goes through
    }
}
```

```rust
// ✅ Good — one spec per format, behavior included; adding a format is a new table entry,
// and no existing code is touched.
pub struct FormatSpec {
    pub format: OutputFormat,
    pub extension: &'static str,
    pub section_alignment: u32,
    /// Builds the path this format's artifact is written to under `target/`.
    pub artifact_path: fn(name: &str, profile: &str) -> String,
}

pub const FORMATS: &[FormatSpec] = &[
    FormatSpec {
        format: OutputFormat::Nro,
        extension: "nro",
        section_alignment: 0x1000,
        artifact_path: |name, profile| format!("target/{profile}/{name}.nro"),
    },
    // ...Nsp, and whatever the loaders accept next
];

pub fn spec_for(format: OutputFormat) -> Option<&'static FormatSpec> {
    FORMATS.iter().find(|spec| spec.format == format)
}
```

2. **Trait objects as the cross-crate seam**
   The bundler's asset surface is open: the bundling crate declares a trait, and every asset-producing module
   supplies an implementation. The bundler never learns what the sources are.

```rust
// ❌ Bad — the bundler imports every asset producer and hardcodes the set.
// The bundling crate now depends on each of them, adding a source edits the bundler,
// and the dependency graph gains a cycle the moment a source wants a bundler type.
pub fn build_bundle(config: &Config) -> Bundle {
    let mut sources = vec![
        Box::new(icon::IconFile::new(&config.icon)) as Box<dyn AssetSource>,
        Box::new(romfs::RomFsDir::new(&config.romfs)),
    ];
    if config.testing {
        sources.push(Box::new(fake_asset::FakeSource::default()));
    }
    Bundle::new(sources)
}
```

```rust
// ✅ Good — the bundler declares the seam; the command composes. Adding a source adds a
// module and one line at the composition root, not an edit to the bundler.
pub trait AssetSource: Send + Sync {
    fn kind(&self) -> AssetKind;
    fn read(&self) -> Result<Vec<u8>, AssetError>;
}

pub struct BundleBuilder {
    sources: Vec<Arc<dyn AssetSource>>,
}

impl BundleBuilder {
    pub fn with_source(mut self, source: Arc<dyn AssetSource>) -> Self {
        self.sources.push(source);
        self
    }
}
```

## Why It Matters

Editing working code to add a variant is how regressions get introduced: the NRO path is tested, the NSP path
is tested, and the third `match` arm added to both is the one that ships broken. A registry entry cannot break
the entries above it. A trait implementation cannot break the implementation next to it.

The three signals make this checkable instead of speculative. A format registry is open because the set of
containers the console accepts keeps growing; a mapping onto the netloader's fixed `i32` ack codes stays a
closed `match`, because the protocol pins the variants and abstracting it would buy nothing.

## Pragmatism Caveat

Not every `match` deserves a registry. If the variant set is frozen by an external spec (the ack codes the
console replies with, the three NRO segment kinds), matched in one or two co-located places, and internal to a
crate, a `match` is clearer and cheaper than an indirection layer. An enum plus `match` also gives you
exhaustiveness checking that a registry table does not: the compiler will not tell you a table entry is
missing.

When a signal fires and you still keep the branching, add a brief comment saying why (the format pins the
variants; the match sites are three lines apart). An undocumented violation is always wrong.

## Checklist

Before committing code, verify:

- [ ] New output formats, asset sources, or compression schemes are added as data entries or trait impls, not
      new branches
- [ ] Per-variant behavior lives in the variant's own entry (a function field or trait impl), not in a shared
      function's `match` arms
- [ ] Crates extend a bundler or packer by handing it a value implementing its trait; the extended crate does
      not import the extending ones
- [ ] The same variant set is not matched on in more than one module, unless the sites are co-located and
      the variant set is frozen by an external spec
- [ ] A retained `match` over a spec-frozen variant set is exhaustive and documented as intentionally closed

## References

- [principle-single-responsibility](principle-single-responsibility.md) - Related: Extension points work only
  when each variant owns one concern
- [principle-dry-wet](principle-dry-wet.md) - Related: Extension points are the right home for genuinely shared
  behavior; flags on a shared helper are not
- [principle-least-surprise](principle-least-surprise.md) - Related: Registry entries must satisfy the same
  behavioral contract callers already expect

## External References

- [Understanding the Open/Closed Principle](https://dev.to/dazevedo/understanding-the-openclosed-principle-ocp-from-solid-keep-code-flexible-yet-stable-jo7)
