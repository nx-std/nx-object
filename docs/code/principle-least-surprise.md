---
name: "principle-least-surprise"
description: "Principle of Least Surprise — code behaves as its name and shape predict; deviations are documented. Load when naming things, designing constructors, or reviewing an API surface"
type: "principle"
scope: "global"
---

# Principle of Least Surprise (Follow Rust Idioms and Conventions)

**MANDATORY for ALL code in the workspace**

## Rule

Code must behave the way a reader predicts from its name, signature, and shape. A name is a contract: what a
function returns, what it touches, and whether it can fail should be guessable without opening it. **The Rust
standard library is the primary reference**: where `std` establishes a pattern for naming, trait usage, or
method semantics, follow it. The conventions here:

1. **Names follow the standard library.** Which constructor, which prefix, which receiver, and what each
   promises about cost and ownership are settled by `std` and collected in the `rust-fn` rule document; this
   document owns the cases that are about behavior rather than naming.
2. **Construction reveals its cost.** A value is never half-initialized: anything that connects, spawns, or
   awaits says so in its name.
3. **Teardown**: the inverse of a named constructor is `shutdown()` or `close()`. Pick one per type, stay
   consistent, and make it safe to call twice.
4. **Paired names**: if the codebase uses `start`/`stop`, do not introduce `begin`/`halt`. The inverse of
   `add` is `remove`, not `delete_item`. The shape the pair shares is owned by `principle-symmetry`; this
   rule owns its vocabulary.
5. **Parameters**: more than two or three related inputs go in a config struct or a builder, never a positional
   `bool`. `pack_nro(elf, assets, true, false)` cannot be reviewed.
6. **No hidden effects**: a function named for a computation does not write to disk, open a connection, or
   mutate global state. Where a lookup and an effect both exist, they are two functions.

## Examples

1. **`new` does not perform I/O; async construction is a named constructor**
   `new` cannot await, so a type that must connect before it is usable cannot be built by one.

```rust
// ❌ Bad — `new` dials the console in the background and returns immediately.
// Every method then needs a "are we connected yet?" guard, and a caller who
// forgets one gets an error from a stream that was never established.
impl NetloaderClient {
    pub fn new(console: ConsoleAddr) -> Self {
        let stream = Arc::new(OnceLock::new());
        tokio::spawn(dial(console, Arc::clone(&stream)));
        Self { stream }
    }
    pub async fn send_nro(&self, name: &str, nro: &[u8]) -> Result<(), SendNroError> {
        let stream = self.stream.get().ok_or(SendNroError::NotConnected)?; // ...on every method
    }
}
```

```rust
// ✅ Good — the named constructor awaits the handshake and hands back a client
// that is, by construction, usable. No method needs a readiness guard.
impl NetloaderClient {
    pub async fn connect(console: ConsoleAddr) -> Result<Self, ConnectError> {
        let stream = TcpStream::connect(console.socket_addr()).await?;
        Ok(Self { stream })
    }
    pub async fn send_nro(&self, name: &str, nro: &[u8]) -> Result<(), SendNroError> {}
}
```

2. **Separate the lookup from the effect**
   Only one of "which packer handles this output format" and "run it" touches the filesystem.

```rust
// ❌ Bad — a name that reads like a query, a body that writes a file. A caller checking
// "is there a packer for this format?" inside a filter silently emits an artifact for
// every format it tests.
pub fn packer_for(format: OutputFormat) -> Option<PackerId> {
    let spec = PACKERS.iter().find(|p| p.handles(format))?;
    spec.write_artifact(); // hidden effect
    Some(spec.id)
}
```

```rust
// ✅ Good — the query is pure; the effect says what it does in its name.
pub fn packer_for(format: OutputFormat) -> Option<&'static PackerSpec> {
    PACKERS.iter().find(|p| p.handles(format))
}

/// Pack the built executable into `format` and write it out. Returns the packer that ran.
pub fn write_artifact(format: OutputFormat, elf: &Elf) -> Result<Option<PackerId>, PackError> {
    let Some(spec) = packer_for(format) else { return Ok(None) };
    spec.pack(elf)?;
    Ok(Some(spec.id))
}
```

## Why It Matters

Every broken convention forces a reader to open the implementation, and across a codebase that cost is paid
mostly in bugs: a caller who assumes `as_string()` is a borrow calls it once per RomFS entry.

Consistency also compounds. Because every resource-owning type here is built by a named async constructor,
`Type::new(..)` tells a reviewer the type owns no resources — or that something is wrong. Standard traits buy
ecosystem integration on top: serde, clap, and `?` all compose with `FromStr`, `From`, and `TryFrom`, while a
custom constructor requires custom glue at every boundary.

## Pragmatism Caveat

A domain term beats a convention when it is genuinely clearer. `pack` on an NRO builder beats `into_packed`
even though it consumes `self`, because pack is the established verb. Prefer the domain word
only when it is _more_ predictable, not merely more clever. Some deviations are imposed from outside: a trait
from a dependency dictates its own method names, so match the foreign convention at the boundary and the
workspace convention everywhere else.

When you deviate deliberately — a method that never fails, a fire-and-forget send, a name a dependency forced —
say why in a doc comment at the declaration. An undocumented deviation is always wrong; the next reader cannot
tell it from a mistake.

## Checklist

Before committing code, verify:

- [ ] Names follow the standard library's vocabulary; the concrete forms are checked against `rust-fn`
- [ ] No value is observable half-initialized; anything that connects or awaits says so in its name
- [ ] Every named constructor that acquires a resource has a matching `shutdown()`/`close()`, and calling
      it twice is safe
- [ ] No function named for a query performs an effect; lookup and effect are separate functions
- [ ] More than two or three related parameters are a config struct or a builder; no positional `bool`
- [ ] Any intentional deviation (a domain verb, a dependency-imposed shape) is documented at the declaration

## References

- [principle-type-driven-design](principle-type-driven-design.md) - Related: The same discipline, for types
- [principle-validate-at-edge](principle-validate-at-edge.md) - Related: `FromStr`/`TryFrom` parse at the edge
- [principle-idempotency](principle-idempotency.md) - Related: `connect`/`shutdown` are safe to call twice
- [principle-single-responsibility](principle-single-responsibility.md) - Related: A type that cannot be named
  in one sentence cannot have a predictable API
- [principle-symmetry](principle-symmetry.md) - Related: A prediction is only available where the same idea
  keeps the same shape

## External References

- [Rust API Guidelines — Naming](https://rust-lang.github.io/api-guidelines/naming.html)
- [Principle of Least Surprise (principles-wiki.net)](https://principles-wiki.net/principles:principle_of_least_surprise)
- [The Principle of Least Astonishment](https://dev.to/notmattlucas/the-principle-of-least-astonishment-3f9k)
- [What is the Principle of Least Astonishment?](https://softwareengineering.stackexchange.com/a/187462)
