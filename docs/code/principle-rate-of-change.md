---
name: "principle-rate-of-change"
description: "Rate of Change — one lifetime per type; keep volatile policy out of stable mechanism and resolve run-constant facts once. Load when a type holds both configuration and runtime state, when deciding where a resolved fact is pinned, or when splitting a module edited on two schedules"
type: "principle"
scope: "global"
---

# Rate of Change (Group by Lifetime, Split What Changes Apart)

## Rule

Things that change at the same rate belong together. Things that change at different rates belong apart, even
when they are about the same subject. "Rate" is measured two ways: how often a **value** is replaced at
runtime, and how often a **line of code** is edited across releases.

1. **One lifetime per type.** Values fixed when the command starts, values that live as long as the process,
   and values that live for one transfer or one packing step are three lifetimes and belong in three types. A
   type that mixes them has to be reasoned about at the fastest rate it contains, and its slow fields acquire
   an `Option` or a lock they did not need.
2. **Resolve a fact at the rate the fact changes, once.** A decision that is fixed for a run is made once at
   the start of the run and carried as a value. Re-deriving it inside a loop means a change landing mid-run
   can be observed halfway through, and the same input stops producing the same output.
3. **Volatile policy does not live inside stable mechanism.** A parser for a format that has not moved since
   the console shipped, and rules that move every release, are edited by different people for different
   reasons. Keep the rules out of the parser and pass it the parsed value.
4. **Rate decides what is cohesive.** This principle overrides topical grouping: two things about the same
   subject that change on different schedules are two concerns, and two near-identical things that change on
   different schedules were never one fact, whatever `principle-dry-wet` would say about the duplication.
5. **Use history as evidence, not intuition.** A file where half the lines were last touched this month and
   half two years ago is naming its own seam. That signal is real; "this might change one day" is not.

## Examples

1. **One lifetime per type**
   A deployer holding its CLI-fixed settings, its process-lifetime resources, and its per-transfer bookkeeping
   in one struct.

```rust
// ❌ Bad — three lifetimes in one type. The two per-transfer fields are meaningless
// between transfers, so they are `Option`, and every method that touches them carries an
// unwrap or a "nothing in flight" branch. Worse, the whole struct is behind a `Mutex`
// because two of six fields mutate, so the stdio forwarder serializes on the settings.
pub struct Deployer {
    discovery_timeout: Duration,     // fixed by the CLI arguments
    retries: u32,                    // fixed by the CLI arguments
    broadcast: UdpSocket,            // process lifetime
    stdio: StdioForwarder,           // process lifetime
    current_file: Option<EntryPath>, // one transfer
    bytes_sent: u64,                 // one transfer
}
```

```rust
// ✅ Good — three types, three lifetimes. `Transfer` is constructed per file, so its
// fields are never absent and never need a lock; `Netloader` is cloneable and shared
// because nothing in it mutates; `DeployConfig` is read once and never again.
pub struct DeployConfig {
    pub discovery_timeout: Duration,
    pub retries: u32,
}

#[derive(Clone)]
pub struct Netloader {
    pub broadcast: Arc<UdpSocket>,
    pub stdio: StdioForwarder,
}

pub struct Transfer {
    pub file: EntryPath,
    pub bytes_sent: u64,
}
```

2. **Resolve once, at the rate the fact changes**
   Which console a deploy sends to is fixed for that deploy; the broadcast socket is fixed for the process.

```rust
// ❌ Bad — the console address is re-discovered per chunk, from a handle that outlives
// the transfer. A console that reboots onto a new address mid-transfer takes the tail of
// the NRO while the head sits on the old one, and the transfer reports success.
async fn send_all(
    ctx: &DeployCtx,
    name: &EntryPath,
    chunks: Chunks,
) -> Result<(), SendNroError> {
    while let Some(chunk) = chunks.next().await {
        let console = ctx.discover().await?;
        ctx.client.send_chunk(console, chunk).await?;
    }
    Ok(())
}
```

```rust
// ✅ Good — the console is resolved once where the deploy is set up and carried as a
// value; the context holds only what lives as long as the process. The transfer lands on
// one device, and the loop cannot observe a change it was not built for.
async fn send_all(
    ctx: &DeployCtx,
    console: ConsoleAddr,
    chunks: Chunks,
) -> Result<(), SendNroError> {
    while let Some(chunk) = chunks.next().await {
        ctx.client.send_chunk(console, chunk).await?;
    }
    Ok(())
}
```

3. **Volatile policy out of stable mechanism**
   An NRO reader and the rules deciding which assets are worth carrying into a bundle.

```rust
// ❌ Bad — the selection rules live inside the reader, so every rule change edits the one
// function that must not break, and the reader's tests grow a fixture per rule. A tweak to
// the skip list that assumes every asset has a non-empty body puts a panic in the function
// every image in the workspace goes through, and the reviewer is reading the policy, not
// the parsing.
pub fn read_nro(image: &[u8]) -> Result<Nro, FromBytesError> {
    let mut assets = Vec::new();
    for raw in asset_entries(image) {
        let asset = Asset::from_bytes(raw)?;
        if asset.kind == AssetKind::Icon && asset.body[0] == 0 {
            continue;
        }
        if SKIPPED_ASSETS.contains(&asset.kind) {
            continue;
        }
        assets.push(asset);
    }
    Ok(Nro { header: NroHeader::from_bytes(image)?, assets })
}
```

```rust
// ✅ Good — the reader only reads, and is edited when the format moves. The policy is a
// separate function over parsed values, edited every release, with its own tests and no
// way to corrupt parsing.
pub fn read_nro(image: &[u8]) -> Result<Nro, FromBytesError> {
    let assets = asset_entries(image)
        .map(Asset::from_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Nro { header: NroHeader::from_bytes(image)?, assets })
}

/// Drop assets the bundle does not carry.
pub fn retain_bundled(nro: &mut Nro) {
    nro.assets.retain(|asset| !asset.is_empty() && !SKIPPED_ASSETS.contains(&asset.kind));
}
```

## Why It Matters

A type is only as easy to reason about as its fastest-changing field. Put one mutable counter beside five
immutable settings and the whole type needs a lock, loses `Clone`, and can no longer be shared; every reader
of the settings now pays for a field they never touch. The `Option` fields are the visible symptom: a field
that is absent outside one phase is a lifetime that wanted its own type, and the phase invariant it should
have carried is instead re-checked at every use.

Re-deriving a slow-changing fact at a fast rate is the version that costs bytes rather than legibility. A
value that was constant for a whole run on the desk is not constant when the console reboots onto a new
address mid-transfer, or when a rebuild replaces the artifact under a bundle step that is still reading it,
and the failure is a run that is internally inconsistent rather than one that stops.

The code side is paid in review. When volatile rules sit inside stable mechanism, every routine policy edit
arrives as a diff against parsing code, and the reviewer either reads the mechanism again or waves the change
through. Separated, the same edit touches a file whose tests are about exactly that question.

## Pragmatism Caveat

Rates are estimates, and splitting on a predicted rate is the same mistake as extracting a premature
abstraction. Split when the two rates are **structural** (per-transfer against process lifetime, compile time
against runtime) or when history already shows them, not because a field looks like it might churn. A type
holding two settings that have never moved independently is one concern until proven otherwise.

Splitting also has a price, and it is threading. If separating a value means passing it through five layers
that have no interest in it, the split may cost more than the coupling it removes; keep them together and say
why in a comment at the type. The same applies where a lifetime split would buy nothing: a small struct built
and dropped in one function does not need its phases separated.

When you knowingly keep two rates together, write the reason at the declaration. An undocumented mix is always
wrong, because the next reader cannot tell a deliberate choice from a type that simply accreted.

## Checklist

Before committing code, verify:

- [ ] No type mixes startup-fixed values, process-lifetime handles, and per-command or per-transfer state
- [ ] No field is `Option` only because it is absent outside one phase
- [ ] Nothing is locked because a minority of its fields mutate
- [ ] Facts that are constant for a run are resolved once at its start and carried as values
- [ ] No loop re-derives a value that cannot legitimately change while the loop runs
- [ ] Rules that change per release are not edited inside code that parses, packs, or writes
- [ ] Values with different sources (compile-time constants, CLI arguments, runtime discovery) have different
      homes
- [ ] Any deliberate mixing of rates carries a comment saying why

## References

- [principle-single-responsibility](principle-single-responsibility.md) - Related: Two rates of change are two
  reasons to change, which is the same split arrived at from the other side
- [principle-dry-wet](principle-dry-wet.md) - Related: Two copies that change on different schedules are two
  facts, however alike they look
- [principle-symmetry](principle-symmetry.md) - Related: Divergence on different schedules is when to break a
  symmetric pair on purpose
- [principle-type-driven-design](principle-type-driven-design.md) - Related: A per-phase type removes the
  `Option` fields that a mixed lifetime forces
- [principle-information-hiding](principle-information-hiding.md) - Related: Splitting by rate is what lets
  the volatile half change without widening the stable half's surface

## External References

- [Rate of Change, in Kent Beck's Implementation Patterns](https://zxuanhong.medium.com/kent-beck-implementation-pattern-principles-6-rate-of-change-4c63354cc84)
- [Tune Software Development for Rate of Change — Kent Beck](https://medium.com/@kentbeck_7670/tune-software-development-for-rate-of-change-not-rate-of-progress-56f93c15a769)
- [Shearing layers](https://en.wikipedia.org/wiki/Shearing_layers)
- [On the Criteria To Be Used in Decomposing Systems into Modules — Parnas](https://dl.acm.org/doi/10.1145/361598.361623)
