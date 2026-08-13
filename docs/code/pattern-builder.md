---
name: "pattern-builder"
description: "Builder pattern for complex object construction with required fields. Load when designing constructors with multiple required parameters or optional configuration"
type: core
scope: "global"
---

# Builder Pattern for Required Fields

**MANDATORY for ALL Rust code in the workspace**

## Rule

Use the builder pattern when construction has multiple required fields. The built type carries no `Option` fields for data that must always be present: the builder holds the optionality, and `build()` enforces completeness. A struct that exposes required data as `Option` because "it is set during construction" leaks its construction concerns into every consumer, and consumers must never unwrap fields that are guaranteed to exist.

## Examples

```rust
// ❌ Bad — easy to forget required fields, consumers deal with Option
pub struct NacpSpec { pub title: Option<AppTitle>, pub author: Option<Author> }

// Every consumer must unwrap or check fields that should always exist
fn write_control(spec: &NacpSpec) {
    let title = spec.title.as_ref().expect("missing title"); // runtime panic risk
}
```

```rust
// ✅ Good — builder enforces completeness, built type has no Option for required fields
pub struct NacpSpec { title: AppTitle, author: Author } // No Option — guaranteed to exist

pub struct NacpBuilder { title: Option<AppTitle>, author: Option<Author> }

impl NacpBuilder {
    pub fn new() -> Self {
        Self { title: None, author: None }
    }

    pub fn title(mut self, title: AppTitle) -> Self {
        self.title = Some(title);
        self
    }

    pub fn author(mut self, author: Author) -> Self {
        self.author = Some(author);
        self
    }

    // Yields a NacpSpec whose title is AppTitle, not Option<AppTitle> — no unwrapping downstream
    pub fn build(self) -> Result<NacpSpec, BuildError> {
        Ok(NacpSpec {
            title: self.title.ok_or(BuildError::MissingTitle)?,
            author: self.author.ok_or(BuildError::MissingAuthor)?,
        })
    }
}
```

```rust
// ✅ Good — type-state builder enforces required fields at compile time
pub struct Missing;
pub struct Set<T>(T);

pub struct NacpBuilder<Title, Author> { title: Title, author: Author }

impl NacpBuilder<Missing, Missing> {
    pub fn new() -> Self { Self { title: Missing, author: Missing } }
}

impl<A> NacpBuilder<Missing, A> {
    pub fn title(self, title: AppTitle) -> NacpBuilder<Set<AppTitle>, A> {
        NacpBuilder { title: Set(title), author: self.author }
    }
}

impl<T> NacpBuilder<T, Missing> {
    pub fn author(self, author: Author) -> NacpBuilder<T, Set<Author>> {
        NacpBuilder { title: self.title, author: Set(author) }
    }
}

// build() exists only once every required field is set — compile-time enforcement
impl NacpBuilder<Set<AppTitle>, Set<Author>> {
    pub fn build(self) -> NacpSpec {
        NacpSpec { title: self.title.0, author: self.author.0 }
    }
}
```

## Why It Matters

Required data represented as `Option` forces every consumer to handle a `None` that should never occur. The builder isolates construction complexity in one place and produces a type that unconditionally guarantees its required fields, removing an entire class of runtime panics from unwrapping "always-present" fields.

## Pragmatism Caveat

Not every struct needs a builder. A struct with 2-3 required fields all available at construction time is clearer with a plain `new()`. Use a builder when construction is genuinely complex: many fields, a mix of required and optional, or an order that matters. Prefer a type-state builder when misuse would be a serious bug; a runtime `build() -> Result` is fine for configuration-style objects where a clear error message suffices.

## Checklist

Before committing code, verify:

- [ ] Built types use concrete fields (not `Option`) for data that must always be present
- [ ] Builder's `build()` method validates all required fields are set
- [ ] Consumers of the built type never unwrap fields that the builder guarantees
- [ ] Simple structs with few required fields use `new()` instead of a builder
- [ ] Type-state builders considered for safety-critical construction where compile-time enforcement is warranted

## References

- [principle-type-driven-design](principle-type-driven-design.md) - Foundation: Design principle this pattern implements
- [pattern-typestate](pattern-typestate.md) - Related: Type-state pattern used for compile-time builder enforcement
