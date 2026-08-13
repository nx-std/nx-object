---
name: "principle-single-responsibility"
description: "Single Responsibility — one struct, one reason to change; split when it spans external systems or mixes I/O with pure computation. Load when designing structs, splitting modules, or reviewing types that do both"
type: "principle"
scope: "global"
---

# Single Responsibility Principle (SRP)

**MANDATORY for ALL code in the workspace**

## Rule

A struct or module owns one responsibility. Split when any of these observable signals is present:

1. **Multiple external systems**: the methods touch several distinct outside worlds (the filesystem, a
   spawned `cargo`, a socket to the console, the terminal). Each boundary is its own concern. Three is a
   strong signal; two warrants a split when they change independently.
2. **Disjoint field access**: the methods partition into groups that touch non-overlapping sets of fields. The
   groups are separate types sharing a struct by coincidence.
3. **Mixed I/O and transformation**: the same unit both performs effects (query, fetch, write) and does pure
   computation (parsing, planning, encoding). Extract the pure part — it is the part worth unit testing, and
   effects make that impossible.

When a signal fires, split into focused units and compose them.

## Examples

1. **One concern per module in a crate**
   The deploy path is four modules, each with exactly one job: discovery (which address is a console, is one
   listening), a client (one console, one connection, one transfer), the packer (bytes in, bytes out), and the
   command that wires them together for the user.

```rust
// ❌ Bad — one struct owns discovery, the connection, the packing, and the terminal output.
// Signals 1 and 2 both fire: it touches UDP + TCP + the filesystem + stdout, and
// `discovered`/`broadcast` are never read by the same methods that read `sock`/`sent`.
pub struct Deployer {
    discovered: Vec<ConsoleAddr>,
    broadcast: UdpSocket,
    sock: Option<TcpStream>,
    sent: usize,
}

impl Deployer {
    pub async fn discover(&mut self) -> Result<(), DiscoveryError> {}
    pub fn console_for(&self, name: &str) -> Option<ConsoleAddr> {}
    pub async fn connect(&mut self, console: ConsoleAddr) -> Result<(), ConnectError> {}
    pub async fn send(&mut self, nro: &[u8]) -> Result<(), SendNroError> {}
    pub fn pack(&self, elf: &Elf) -> Result<Vec<u8>, PackError> {}
    pub fn print_progress(&self) {}
}
```

```rust
// ✅ Good — four units, each describable in one sentence, composed by the caller.
// The discovery module — "which address is a console, and is one listening"
pub async fn discover(timeout: Duration, retries: u32) -> io::Result<Option<ConsoleAddr>>;

// The client module — "one console, one connection, one transfer"
pub struct NetloaderClient { /* ... */ }

impl NetloaderClient {
    pub async fn connect(console: ConsoleAddr) -> Result<Self, ConnectError>;
    pub async fn send_nro(&mut self, name: &str, nro: &[u8]) -> Result<(), SendNroError>;
}

// The packer module — "an ELF in, an NRO out"; no I/O at all
pub fn pack_nro(elf: &Elf, assets: Option<Assets>) -> Result<Vec<u8>, PackError>;

// The command module — "wire the three together and report to the user"
pub struct DeployCommand { /* ... */ }
```

2. **Separate the pure transformation from the effect**
   Writing an image splits in two: one function does the filesystem I/O, another is a pure layout function.
   Only the pure one holds the tricky invariant (every entry starts on an aligned offset), and only the pure
   one can be unit tested directly.

```rust
// ❌ Bad — the alignment rule is trapped inside the I/O. Testing "every entry starts
// aligned" now requires a real directory tree and a written file, and a version that
// packed entries at their natural offset produced images whose name table read back
// shifted — on hardware only, where no test was looking.
pub fn write_image(dir: &Path, out: &Path) -> Result<u64, WriteImageError> {
    let mut file = std::fs::File::create(out)?;
    let mut offset = 0;
    for entry in std::fs::read_dir(dir)? {
        let contents = std::fs::read(entry?.path())?;
        file.write_all(&contents)?;
        offset += contents.len();
    }
    Ok(offset as u64)
}
```

```rust
// ✅ Good — the effect is a thin shell over a pure core.
/// Lay out `entries` in the image, aligning each to `ENTRY_ALIGNMENT`.
///
/// An entry never starts mid-alignment-unit: the loader reads the name table by offset.
pub fn plan_layout(entries: &[EntryMeta]) -> Vec<EntryOffset> {
    // pure: metadata in, offsets out — tested with a slice, no filesystem
}

/// Write each planned entry to `out`. Returns the image size.
pub fn write_image(dir: &Path, out: &Path) -> Result<u64, WriteImageError> {
    let entries = scan_entries(dir)?;
    let mut file = std::fs::File::create(out)?;
    for (entry, offset) in entries.iter().zip(plan_layout(&entries)) {
        write_entry(&mut file, entry, offset)?;
    }
    Ok(file.metadata()?.len())
}
```

## Why It Matters

A type with one responsibility has one reason to change. A client changing its retry strategy cannot break
image layout, because it does not contain any. Discovery gaining an address family cannot break the transfer
handshake.

The testability consequence is concrete: anything fused to an effect can only be exercised by an integration
or e2e test, and those need a temp tree, a toolchain, and sometimes a console — so they run in CI, not in the
edit loop, and the cases that are awkward to set up end up untested. Separable pure logic is tested with a
slice and an assertion, in milliseconds, at the point of the change.

## Pragmatism Caveat

Small structs that touch two systems are not automatically wrong. A transfer that owns both the compressor's
buffer and the socket is one concern, because the whole point of the type is the coupling between them (never
stall the compressor, keep the socket fed) — splitting them would put the invariant in neither half. A deploy
command owns its stdio forwarder because the forwarder's lifetime _is_ the command's lifetime.

When a signal fires and you keep the concerns together, say why in the module `//!` docs or a comment
(transactional atomicity, a shared invariant, a lifetime that cannot be split). An undocumented violation is
always wrong.

## Checklist

Before committing code, verify:

- [ ] Each struct or module can be described in one sentence without "and"
- [ ] No type both performs I/O and holds a non-trivial pure algorithm — the algorithm is a free function
- [ ] Fields partition into one cohesive group, not two groups touched by disjoint method sets
- [ ] A change to one concern (discovery, wire format, image layout) touches one module
- [ ] Deliberate co-location of concerns is explained in the module docs

## References

- [principle-law-of-demeter](principle-law-of-demeter.md) - Related: Overloaded types are the ones callers end
  up navigating through
- [principle-open-closed](principle-open-closed.md) - Related: Extension points require variants that each own
  one concern
- [principle-inversion-of-control](principle-inversion-of-control.md) - Related: Separating pure logic is what
  makes injecting collaborators worthwhile
- [principle-dry-wet](principle-dry-wet.md) - Related: An abstraction serving two responsibilities is the wrong
  abstraction
- [principle-rate-of-change](principle-rate-of-change.md) - Related: Two rates of change are two reasons to
  change; the same split reached from the other side

## External References

- [SOLID: The Single Responsibility Principle (Uncle Bob)](https://blog.cleancoder.com/uncle-bob/2014/05/08/SingleReponsibilityPrinciple.html)
- [Single Responsibility Principle with a Rust Example](https://medium.com/@dogabudak/single-responsibility-principle-with-a-rust-example-2940504e3ebd)
