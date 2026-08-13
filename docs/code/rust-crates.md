---
name: "rust-crates"
description: "Cargo.toml section ordering, feature flag rules, kebab-case naming. Load when editing Cargo.toml or adding features to a crate"
type: arch
scope: "global"
---

# Rust Crate Manifest Patterns

**MANDATORY for ALL `Cargo.toml` files in this workspace**

## 1. Cargo.toml Section Ordering

Sections MUST appear in this exact order, with no other section mixed between them:

1. `[package]` — crate metadata
2. **Target definitions** — `[lib]`, `[[bin]]`, `[[bench]]`: what this manifest builds
3. `[features]` — feature flags and their dependencies
4. `[dependencies]` — runtime dependencies
5. `[dev-dependencies]` — development and test dependencies
6. `[build-dependencies]` — build-time dependencies, and `[target.'cfg(…)'.build-dependencies]` in the same
   slot, since it is the same section under a gate
7. `[lints.<tool>]` — lint configuration

Every section except `[package]` is optional. Dependencies within each section MUST be alphabetically ordered,
and sections are separated by a blank line.

The order reads as **what the crate is, what it builds, what it needs, and how it is checked**. Target
definitions sit high because they answer "is this a library, a binary, or both" — the question a reader asks
before any dependency matters. Lints sit last because they configure the build rather than compose it, and a
cfg-gated `[lints.rust]` declaring a `check-cfg` belongs beside the gate it names.

```toml
# ✅ Good — a reviewer knows where to look for a new dependency, and two branches adding
# one each conflict on different lines instead of the same one.
[package]
name = "nx-object"
version = "0.1.0"
edition = "2024"

[lib]
name = "nx_object"
crate-type = ["rlib"]
bench = false

[features]
default = []

[dependencies]
bitflags = "2.9"
thiserror = { version = "2.0", default-features = false }
zerocopy = { version = "0.8", default-features = false, features = ["derive"] }

[dev-dependencies]
tempfile = "3.0"

[target.'cfg(gen_schemas)'.build-dependencies]
schemars = "0.8"

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(gen_schemas)"] }
```

Named dependency groups are allowed when a set of dependencies has shared external update ownership, lockstep
compatibility requirements, or operational tooling rules such as Renovate groups. Each group MUST carry a
concise comment explaining why it is grouped, and dependencies MUST stay alphabetically ordered inside it.

## 2. Features Section Rules

**Features sections are OPTIONAL. Do NOT add a `[features]` section if the crate doesn't already have one. The
`default` feature is implicit and optional when empty.**

When a `[features]` section exists:

- All features MUST be ordered alphabetically. The one exception: `default` MUST be listed FIRST.
- Feature names MUST use kebab-case — lowercase letters and hyphens only.
- Names MUST be descriptive rather than abbreviated. `elf-parsing` says what it enables; `elf` does not say
  whether it adds a parser, a writer, or both. Likewise `lz4-compression` over `lz4`, `json-serialization`
  over `json`, `filesystem-support` over `std`.
- Every feature MUST have a `#` comment above it explaining its purpose.

```toml
# ❌ Bad — unordered, undocumented, and named so that no reader can tell what turning one
# on actually pulls in; `elf` was assumed to include the writers and shipped without them.
[features]
lz4 = ["dep:lz4_flex"]
filesystem-support = ["dep:fs-err"]
default = ["filesystem-support"]
ELF_parsing = ["dep:object"]
# stuff
elf = ["dep:object"]
```

```toml
# ✅ Good — `default` first, the rest alphabetical, kebab-case, and each line says what
# enabling it buys.
[features]
# Default features, enabled unless default-features = false
default = []
# ELF parsing, for deriving NRO and NSO segments from a linked binary
elf-parsing = ["filesystem-support", "dep:object"]
# Filesystem-backed write layer: the format builders and their output paths
filesystem-support = ["dep:fs-err", "dep:sha2", "serde/std"]
# LZ4 compression and decompression of NSO segments
lz4-compression = ["filesystem-support", "dep:lz4_flex"]
```

## Checklist

Before committing Cargo.toml changes, verify:

- [ ] Sections appear in the correct order: `[package]` → target definitions → `[features]` → `[dependencies]`
      → `[dev-dependencies]` → `[build-dependencies]` → `[lints.<tool>]`
- [ ] All dependencies within each section are alphabetically ordered, or split into documented named groups with alphabetical ordering inside each group
- [ ] Features use kebab-case naming
- [ ] `default` feature is listed first (if present)
- [ ] All remaining features are alphabetically ordered
- [ ] Every feature has a descriptive `#` comment above it
- [ ] No `[features]` section added unnecessarily

## References

- [principle-information-hiding](principle-information-hiding.md) - Foundation: Why a feature gate is part of
  the crate's public surface, not an implementation detail
