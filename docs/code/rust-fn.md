---
name: "rust-fn"
description: "Function names and signatures against std: new is infallible, create is fallible, Self names the enclosing type, as_/to_/into_ predict cost and ownership. Load when naming a function or choosing a receiver"
type: "core"
scope: "global"
---

# Function Names and Signatures

**MANDATORY for ALL Rust code in the workspace**

**The standard library is the reference.** A name that matches `std` needs no documentation; one that
contradicts it costs a trip to the source. Where `std` establishes a convention for a name, a receiver, or a
return type, follow it.

The principle behind it, and the cases about behavior rather than naming, belong to
[principle-least-surprise](principle-least-surprise.md).

## 1. Constructors

`new` is **infallible, cheap, and free of I/O**. It takes the values the type needs and returns the type —
not `Result`, not a half-built value, and never after a network round trip. When construction can fail, the
constructor is named `create`, and the different name is the point: the caller sees at the call site that
this one has a failure path.

| Situation                                   | The constructor                                     |
|---------------------------------------------|-----------------------------------------------------|
| Always succeeds, no I/O                     | `new`                                               |
| Can fail, built from the type's own inputs  | `create`, returning `Result`                        |
| Infallible conversion from another type     | `From` (which yields `Into` for free)               |
| Fallible conversion from another type       | `TryFrom` / `TryInto`                               |
| Parsed from a string                        | `FromStr` ([rust-parse](rust-parse.md))             |
| Needs to await, connect, or spawn           | A named async constructor: `connect`, `open`, `start` |
| Many fields, some optional                  | A builder ([pattern-builder](pattern-builder.md))   |

`create` and `TryFrom` are not interchangeable. `TryFrom` converts **another type** into this one and is what
`?` and `.try_into()` reach for; `create` builds from the type's own inputs — several arguments, a validated
payload, a parsed manifest — where there is no single source value to convert from.

```rust
// ✅ Good — the name says a connection happens, so its cost and its failure are no
// surprise. As `new` this would fail, block, and time out under a name that promises
// none of that, and every caller would carry a `?` for a construction they expect to
// be total.
impl NetloaderClient {
    pub async fn connect(console: SocketAddr) -> Result<Self, ConnectError> {
        let stream = TcpStream::connect(console).await?;
        Ok(Self { stream })
    }
}
```

`new` and `create` never trade places: a `create` that cannot fail is a longer name for `new`. `make` is not
used at all, and `build` is reserved for the terminal method of a builder.

## 2. `Self` Names the Enclosing Type

Inside an `impl` block the type being implemented is written `Self`: in the return position, inside the `Ok`
of a `Result`, and in the struct or enum literal the body builds. Spelling the type's own name there is a
second copy of what the `impl` header already states.

The cost is paid at rename. `Self` makes an `impl` block rename-neutral, so changing a type's name touches the
header and nothing else. Repeating the name spreads the same identifier across every constructor signature and
every literal, and a one-word rename then lands as a diff over the whole block, burying whatever real change
ships alongside it. `Self` also reads shorter at the point that matters: `Result<Self, ConnectError>` says
"this type or an error" without the reader matching a name against the header above it.

The rule covers the enclosing type only. A method that returns a different type names that type.

```rust
// ❌ Bad — the type name is repeated four times in two methods; renaming `Pfs0Builder`
// rewrites all four lines, so the rename commit no longer shows what else it changed.
impl Pfs0Builder {
    pub fn new(alignment: usize) -> Pfs0Builder {
        Pfs0Builder { alignment, entries: Vec::new() }
    }

    pub fn create(dir: &Path) -> Result<Pfs0Builder, ScanError> {
        Ok(Pfs0Builder {
            alignment: DEFAULT_ALIGNMENT,
            entries: scan_entries(dir)?,
        })
    }
}
```

```rust
// ✅ Good — the `impl` header is the only place the name appears, so a rename is a
// one-line diff and the block moves unchanged.
impl Pfs0Builder {
    pub fn new(alignment: usize) -> Self {
        Self { alignment, entries: Vec::new() }
    }

    pub fn create(dir: &Path) -> Result<Self, ScanError> {
        Ok(Self {
            alignment: DEFAULT_ALIGNMENT,
            entries: scan_entries(dir)?,
        })
    }
}
```

Trait impls hold to the same rule, which is also what `std` writes: `fn from(value: Raw) -> Self`,
`fn try_from(value: Raw) -> Result<Self, Self::Error>`, `fn default() -> Self`.

## 3. Conversions Predict Cost and Ownership

The three conversion prefixes are a promise about what a call costs and what it does to the receiver. Getting
one wrong is not a style issue: a caller reads `as_` and puts it in a per-row loop.

| Prefix   | Receiver | Cost                  | Returns                          |
|----------|----------|-----------------------|----------------------------------|
| `as_*`   | `&self`  | Free; never allocates | A borrowed view (`&str`, `&[u8]`)|
| `to_*`   | `&self`  | May allocate          | A new owned value                |
| `into_*` | `self`   | Free or near-free     | A different owned type           |

```rust
// ❌ Bad — `as_` on something that allocates, and `to_` on something that consumes
// the receiver; the second only surfaces when a caller uses the value afterwards.
impl BuildId {
    pub fn as_string(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl NpdmSpec {
    pub fn to_descriptor(self) -> NpdmDescriptor {}
}
```

```rust
// ✅ Good — each prefix matches its receiver and its cost.
impl BuildId {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_hex_string(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl NpdmSpec {
    pub fn into_descriptor(self) -> NpdmDescriptor {}
}
```

`into_inner` is the conventional name for handing back the value a wrapper wraps, and a newtype that hides its
field provides it rather than making callers reach for `.0`. The iterator trio follows the same rule:
`iter(&self)`, `iter_mut(&mut self)`, `into_iter(self)`.

## 4. Predicates Return `bool`

`is_*` and `has_*` return `bool`, and nothing else.

```rust
// ❌ Bad — `is_` returning data. `if nro.has_assets()` does not compile, so the
// caller writes `.is_some()` and the name has bought nothing; and a caller who
// wants the header cannot get it without re-reading the field.
impl Nro<'_> {
    pub fn has_assets(&self) -> Option<&NroAssetHeader> {
        self.asset_header
    }
}
```

```rust
// ✅ Good — a predicate and an accessor, each doing one thing.
impl Nro<'_> {
    pub fn has_assets(&self) -> bool {
        self.asset_header.is_some()
    }

    pub fn asset_header(&self) -> Option<&NroAssetHeader> {
        self.asset_header
    }
}
```

A collection that has `len` also has `is_empty`. `std` pairs them, clippy expects them, and a caller writing
`len() == 0` is a sign the pair is missing.

## 5. `try_` Is the Fallible Variant

`try_*` names the fallible sibling of an operation that otherwise panics or is total: `try_reserve`,
`try_into`. It is **not** a way to name a fallible constructor — that is `create` ([§1](#1-constructors)), and
`try_new` would promise an infallible `new` alongside it.

The prefix is only correct when the un-prefixed operation exists. A function with no infallible counterpart is
not `try_something` — it is `something` returning `Result`, and the prefix would promise a sibling that does
not exist.

## 6. Accessors Are Bare Nouns

A field accessor is named for the thing it returns: `len()`, `name()`, `endpoint()` — never `get_len`,
`get_name`. `std` reserves `get` for fallible or key-based lookup (`slice::get`, `HashMap::get`), where the
return is an `Option` and the call can miss. A `get_` prefix on an infallible field read pushes every caller
to double-check a lookup that cannot fail.

Mutating setters take `&mut self` and are named `set_*`. A method that returns a modified copy instead is
`with_*`, and it takes `self`.

## 7. The Receiver Matches the Prefix

The receiver is part of the promise. `as_*` and `to_*` borrow; `into_*` and `with_*` consume; `set_*` takes
`&mut self`. A method that consumes `self` under a borrowing name forces callers into clones they should not
need; one that borrows under a consuming name leaves a value alive the caller expected to have given away.

Consuming is not a cost to avoid. Taking `self` is how a type moves through a pipeline without a clone, and it
is what makes an invalid intermediate state unconstructible — see [pattern-typestate](pattern-typestate.md).

## 8. The Point Is an Idiomatic Function

None of the rules above is house style. Each is a convention `std` already established, and together they
produce **a function a Rust developer can use correctly without reading its body.** That buys two things.

**The reader predicts the function from its signature.** `fn as_str(&self) -> &str` is free, borrowing, and
infallible before anyone opens it, and `fn to_owned_id(self) -> Id` is visibly wrong without knowing the type,
because the prefix and the receiver disagree.

**The type composes with the ecosystem.** `From` and `TryFrom` give `?` and `.into()`; `FromStr` gives clap's
`value_parser` and serde's `try_from`; `is_empty` alongside `len` satisfies clippy; `into_iter` puts the type
in a `for` loop. A custom name has to be wired to every one of these by hand, at every boundary, forever.

When a domain term is genuinely clearer than the convention, take it — `pack` on a bundle builder beats
`into_packed` even though it consumes `self`, because packing is the established verb for the format. That is a deliberate
trade, documented at the declaration ([principle-least-surprise](principle-least-surprise.md)). What is never
acceptable is a name that reads like a convention and does something else.

## Checklist

Before committing code, verify:

- [ ] `new` is infallible, cheap, and performs no I/O; anything that awaits or connects has a name that says so
- [ ] A fallible constructor is `create` returning `Result`, never a `new` that returns `Result`
- [ ] `TryFrom` is used where there is a single source value to convert; `create` where there is not
- [ ] No `make` was introduced, and `build` appears only as a builder's terminal method
- [ ] Inside an `impl`, the enclosing type is written `Self` in return types, in `Result`, and in literals
- [ ] `as_*` borrows without allocating, `to_*` returns an owned value, `into_*` consumes `self`
- [ ] A wrapper exposes `into_inner` rather than leaving callers to reach for `.0`
- [ ] `is_*` and `has_*` return `bool`; a type with `len` also has `is_empty`
- [ ] `try_*` is used only where the infallible counterpart exists
- [ ] Field accessors are bare nouns; `get` is reserved for lookups that can miss
- [ ] Every receiver matches its prefix: borrowing names take `&self`, consuming names take `self`

## References

- [principle-least-surprise](principle-least-surprise.md) - Foundation: Why a name is a contract, and the
  behavioral cases that are not about naming
- [rust-parse](rust-parse.md) - Related: `FromStr` as the parsing constructor, and its call sites
- [pattern-newtype](pattern-newtype.md) - Related: The wrappers that need `as_*` and `into_inner`
- [pattern-builder](pattern-builder.md) - Related: Where `build` is the correct terminal name
- [pattern-typestate](pattern-typestate.md) - Related: Consuming receivers as a correctness tool
