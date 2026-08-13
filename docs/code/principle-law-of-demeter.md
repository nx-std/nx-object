---
name: "principle-law-of-demeter"
description: "Law of Demeter — a unit talks to its immediate collaborators, never through them to reach something further. Load when reviewing call chains, field access patterns, or coupling concerns"
type: "principle"
scope: "global"
---

# Law of Demeter (Principle of Least Knowledge)

**MANDATORY for ALL code in the workspace**

## Rule

A function or method may only talk to its immediate collaborators. Do not reach through chains of values to get
at something buried deep in the graph. A method `m` of a type `T` may only call methods on `T` itself (`self`),
on values passed as arguments to `m`, on values `m` created, and on values held in `T`'s own fields.

If you write `a.b().c().do_something()` — or, with public fields, `a.b.c.do_something()` — you are violating
the principle. Stop at `a.b()`: if you need something from `c`, ask `a` (or `b`) to hand you the value, or
accept the value as a parameter.

**Not violations**: chains where every link is the same logical value. Builder chains
(`NroBuilder::new().text(text).rodata(rodata).build()`), iterator adapters
(`entries.iter().filter(..).map(..).collect()`), `Result`/`Option` combinators (`.map_err(..).context(..)`), and
matching on an enum a direct collaborator returned (`let lease = pool.lease_for(&console).await?;
lease.client().send_nro(..)`) are not reach-through — the enum is that collaborator's own return value.

## Examples

1. **Ask the collaborator, don't navigate its internals**
   A pool owns a `HashMap<ConsoleName, Arc<NetloaderClient>>`, the key scheme, and the "is this console
   configured" decision.

```rust
// ❌ Bad — reaches through the pool into its map and through the client into its socket.
// This caller now depends on the key being the console name (not its address), on clients
// being stored as Arc, and on NetloaderClient exposing a raw stream. Any of the three
// changing breaks it, and nothing in the type system says this caller exists.
async fn deploy(pool: &ConsolePool, console: &ConsoleSpec, nro: &[u8]) -> Result<(), SendNroError> {
    let client = pool.clients.get(&console.name).expect("configured");
    client.stream.write_all(nro).await.map_err(SendNroError::Io)
}
```

```rust
// ✅ Good — one call to the immediate collaborator, which answers the question completely.
// This caller knows two things: ask the pool for a lease, or report why it cannot have one.
async fn deploy(pool: &ConsolePool, console: &ConsoleSpec, nro: &[u8]) -> Result<(), SendNroError> {
    match pool.lease_for(console).await? {
        Lease::Ready(client) => client.send_nro(&console.name, nro).await,
        Lease::Unconfigured(reason) => Err(SendNroError::Unconfigured(reason)),
    }
}
```

2. **Receive the value, not the object graph that contains it**
   A resolver needs an assets directory and a way to read files. It should take exactly those two things.

```rust
// ❌ Bad — the resolver is handed the whole build context and digs for what it needs.
// It is coupled to the context's shape three levels down, and it cannot be unit tested
// without standing up a full context: workspace metadata, target spec, ui sink and all.
struct AssetResolver {
    ctx: Arc<BuildContext>,
}

impl AssetResolver {
    fn resolve(&self, entry: &EntryPath) -> Option<PathBuf> {
        let root = &self.ctx.workspace.package.config.assets_dir; // three levels of reach-through
        // ...
    }
}
```

```rust
// ✅ Good — declare the two collaborators the resolver actually uses.
// The caller that already holds the build context does the navigation once, at the seam.
// Tests construct this with a temp dir and a two-line closure.
struct AssetResolver {
    assets_dir: PathBuf,
    read_file: Arc<dyn Fn(&Path) -> std::io::Result<Vec<u8>> + Send + Sync>,
}

impl AssetResolver {
    fn resolve(&self, entry: &EntryPath) -> Option<PathBuf> { /* uses only its own two fields */ }
}
```

## Why It Matters

Reach-through chains turn a private implementation detail into a public contract by accident. When a pool
changes its cache key from the console name to `(name, addr)`, every caller that read its map breaks — and
nothing in the type system told you those callers existed. Keeping to immediate collaborators lets a crate
restructure its internals as long as its methods keep their meaning.

The second cost is testability: a type that navigates `ctx.workspace.package.config.assets_dir` can only be
exercised by building a whole build context, so it ends up covered by an end-to-end test or not at all. A type
that takes a `PathBuf` and a read closure is tested with a temp dir and three lines.

## Pragmatism Caveat

A short reach-through is occasionally the honest choice: navigating a plain data structure you own (a
deserialized config, a payload you just validated) is reading data, not coupling. The rule targets navigation
through _behavioral_ values that could hide their internals. When you deliberately reach through one, add a
comment explaining why the alternatives (a delegating method on the direct collaborator, or passing the value
in) were rejected. An undocumented violation is always wrong.

## Checklist

Before committing code, verify:

- [ ] No expression navigates two or more levels into another type's fields to reach behavior
- [ ] Functions accept the values they use (a path, a reader, a handle) rather than a container to dig through
- [ ] Cross-crate access goes through public functions and methods, never through another crate's internal
      collections or state
- [ ] Fluent chains on one logical value (builders, iterators, `Result`/`Option` combinators, matched enums) are
      not mistaken for violations
- [ ] Any deliberate reach-through is local and carries a comment with its rationale

## References

- [principle-single-responsibility](principle-single-responsibility.md) - Related: A type that must be navigated
  deeply usually owns too much
- [principle-inversion-of-control](principle-inversion-of-control.md) - Related: Injecting the value you need is
  the standard cure for reach-through
- [principle-type-driven-design](principle-type-driven-design.md) - Related: Returning an enum lets a
  collaborator answer a question completely instead of exposing its internals

## External References

- [Law of Demeter — Principle of Least Knowledge](https://dev.to/dazevedo/law-of-demeter-principle-of-least-knowledge-35l2)
