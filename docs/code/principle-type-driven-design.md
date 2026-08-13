---
name: "principle-type-driven-design"
description: "Type-Driven Design — make illegal states unrepresentable with enums and newtypes. Load when designing data types or reviewing optional fields that allow invalid combinations"
type: "principle"
scope: "global"
---

# Type-Driven Design (Make Illegal States Unrepresentable)

**MANDATORY for ALL code in the workspace**

## Rule

Design types so invalid states cannot be constructed. Parse at the boundary, then let every downstream function
receive a type that structurally rules out the cases it does not handle.

Concretely:

- A struct with several `Option` fields where only some combinations are legal is wrong. Replace it with an
  **enum** whose variants are exactly the legal shapes.
- Model "succeeded, or here is why not" as an **enum returned by value** or a `Result` with a typed error enum
  — not as `Option<T>` plus an out-of-band message, and not as a string the caller is expected to inspect.
- Let the compiler enforce it. Match exhaustively rather than with a catch-all arm on an enum you own: the
  catch-all is what silently absorbs the variant added next year. Index with `.get(i)` when the index is
  data-derived; `slice[i]` asserts a bound the type does not carry.
- A newtype's invariant is established in its validating constructor — `FromStr` from a string, `TryFrom`
  from any other type — not by the caller. Constructing one from a raw value with
  an unchecked constructor asserts the fact the newtype exists to prove, so those constructors carry a
  `// SAFETY:` comment naming why the invariant already holds.

If a function starts with defensive checks for a state that "shouldn't happen", the type is letting it happen.

## Examples

1. **Enum over co-optional fields**
   A pool answers "give me a client for this console, or tell the caller why it can't have one". Both-set and
   neither-set are meaningless.

```rust
// ❌ Bad — four representable states, two of them nonsense (both set; neither set).
// Every caller must defensively check both fields, and nothing forces it to. A caller
// that reads `client` first panics on the unconfigured path, and no type says so.
pub struct LeaseResult {
    pub client: Option<Arc<NetloaderClient>>,
    pub unavailable: Option<String>,
}

let result = pool.lease_for(console).await;
if let Some(reason) = result.unavailable {
    return Err(SendNroError::Unconfigured(reason));
}
let client = result.client.unwrap(); // the compiler cannot help here
```

```rust
// ✅ Good — exactly two states; the match gives the caller a non-optional client.
pub enum Lease {
    Ready(Arc<NetloaderClient>),
    Unconfigured { reason: String },
}

let client = match pool.lease_for(console).await {
    Lease::Ready(client) => client,
    Lease::Unconfigured { reason } => return Err(SendNroError::Unconfigured(reason)),
};
```

2. **Parse once into a newtype; do not re-assert the invariant downstream**
   A title id is not a `String`. Validate it where it enters and carry the proof in the type.

```rust
// ❌ Bad — a raw String for validated domain data. Is it 0x-prefixed? Lowercased?
// 16 hex digits? Every function downstream either re-checks or trusts blindly, and
// this one panics on any input the caller did not happen to normalize.
pub fn program_id_field(title_id: String) -> String {
    let stripped = title_id.strip_prefix("0x").unwrap();
    format!("{stripped:0>16}")
}
```

```rust
// ✅ Good — one validating constructor; downstream signatures state what they require.
pub struct TitleId(u64);

impl std::str::FromStr for TitleId {
    type Err = ParseTitleIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let digits = input.strip_prefix("0x").unwrap_or(input);
        let value = u64::from_str_radix(digits, 16).map_err(ParseTitleIdError::Malformed)?;
        Ok(Self(value))
    }
}

// No defensive check: the type already carries the proof.
pub fn program_id_field(title_id: TitleId) -> String {
    format!("{title_id:016x}")
}
```

## Why It Matters

Every illegal state a type permits becomes a defensive check somewhere — or, more often, a missing defensive
check and a panic halfway through a build. `client: Option<Arc<NetloaderClient>>` pushes an absence check onto
every call site; an enum forces the caller to handle the failure _once_, at the match, and hands them a
non-optional value afterwards.

It also decides what your errors can say. A `String` that might be a title id produces "invalid input" from
somewhere deep inside NPDM generation; a `TitleId` that failed to parse produces a typed error at the edge,
with the offending value and the metadata key that carried it, before a single byte was packed.

## Pragmatism Caveat

Encode structural invariants, not policy. "An NRO either carries an asset section or it does not, never half of
one" is structural — put it in the type. "Discovery retries three times" is policy that will change — keep it a
runtime value.

The same test decides when a primitive earns a newtype. A value with an invariant, a unit, or a same-typed
sibling it must never be swapped with is structural: a file offset and a memory offset, a 0-based segment index
and a 1-based line, a source path and the entry path derived from it. Those get newtypes, validated in
`FromStr` and constructed at the boundary the value enters through — the `pattern-newtype` rule document
governs them. A value with no invariant and nothing to confuse it with — a label, a free-form message —
stays a plain `String`; a newtype there is ceremony.

Casting **into** a validated type is the same error wearing a nominal type: `EntryPath(raw_string)` from an
unvalidated source asserts exactly what the newtype exists to prove. Where an unchecked constructor is
genuinely warranted, it carries a `// SAFETY:` comment naming the reason the invariant already holds.

## Checklist

Before committing code, verify:

- [ ] No struct has two or more `Option` fields whose combinations include meaningless states
- [ ] "Succeeded or here's why not" is an enum or a `Result` with a typed error, not `Option` plus a message
- [ ] Matches on enums the workspace owns are exhaustive, not closed with a catch-all arm
- [ ] Data-derived indexing uses `.get()` and discharges the absence; `slice[i]` is used only where the bound
      is structurally guaranteed
- [ ] A primitive with an invariant, a unit, or a same-typed sibling it must not be swapped with is a newtype;
      one with neither stays plain
- [ ] Newtype invariants are established in `FromStr` or `TryFrom`; every unchecked constructor carries a
      `// SAFETY:` comment
- [ ] No defensive runtime check re-verifies something the type already guarantees

## References

- [principle-validate-at-edge](principle-validate-at-edge.md) - Related: Boundary parsing is what produces the
  validated types this principle relies on
- [principle-least-surprise](principle-least-surprise.md) - Related: A well-named type whose shape lies is worse
  than no type
- [principle-law-of-demeter](principle-law-of-demeter.md) - Related: Returning an enum lets a collaborator
  answer completely instead of exposing internals

## External References

- [Designing with Types: Making Illegal States Unrepresentable (F# for Fun and Profit)](https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/)
- [Parse, Don't Validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
- [Parse, Don't Validate and Type-Driven Design in Rust](https://www.harudagondi.space/blog/parse-dont-validate-and-type-driven-design-in-rust/#maxims-of-type-driven-design)
- [The Ultimate Guide to Rust Newtypes](https://www.howtocodeit.com/guides/ultimate-guide-rust-newtypes)
- [Using Types To Guarantee Domain Invariants](https://lpalmieri.com/posts/2020-12-11-zero-to-production-6-domain-modelling/)
