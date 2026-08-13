---
name: "principle-inversion-of-control"
description: "Inversion of Control — accept dependencies as parameters instead of constructing them. Load when designing components, wiring collaborators, or making code testable without external systems"
type: "principle"
scope: "global"
---

# Inversion of Control (Dependency Injection)

**MANDATORY for ALL code in the workspace**

## Rule

A unit declares what it needs; it does not go and find it. Pass collaborators in — as function parameters, as
struct fields set by the constructor, or as generic parameters — instead of constructing them inside.

Inject when either signal fires:

1. **It performs I/O**: it opens a socket, reads or writes files, prints to the terminal, or reads the clock.
   Tests must be able to substitute it.
2. **It varies by context**: production and tests (or one deployment mode and another) need different
   instances.

If neither fires — a `Vec`, a `HashMap`, a pure helper in the same module — construct it inline. Injecting it
is noise.

Rust gives two shapes for the seam, and they are not interchangeable. A generic parameter (`W: AsyncWrite`)
monomorphizes, keeps the call devirtualized, and is the default for a collaborator fixed at construction. A
trait object (`Arc<dyn Packer>`) is for sets assembled at runtime or stored heterogeneously — the composition
root's registry of packers, not a struct's single collaborator.

## Examples

1. **Inject the effect, not the machinery that performs it**
   A RomFS build needs the files under a directory. It takes a scanner, not a directory walk.

```rust
// ❌ Bad — the directory walk is baked in. To test "an empty tree produces an empty image"
// the test must create a temp directory; to test nesting it must lay a tree out on disk,
// and to test the too-many-entries case it must create thousands of files. None of those
// tests are about the filesystem.
pub fn build_romfs(root: &Path) -> Result<Vec<u8>, BuildRomFsError> {
    let mut builder = RomFsBuilder::new();
    for file in scan_disk(root)? {
        builder.add_file(file)?;
    }
    builder.build()
}
```

```rust
// ✅ Good — the one effect is a parameter. Production passes the real scanner; tests pass a
// closure returning in-memory entries and assert on the image bytes, with no temp directory.
pub fn build_romfs(
    root: &Path,
    scan: impl Fn(&Path) -> std::io::Result<Vec<RomFsFile>>,
) -> Result<Vec<u8>, BuildRomFsError> {
    let mut builder = RomFsBuilder::new();
    for file in scan(root)? {
        builder.add_file(file)?;
    }
    builder.build()
}

pub fn scan_disk(root: &Path) -> std::io::Result<Vec<RomFsFile>> { /* walks the directory */ }

// The test — no framework, just a closure:
let two_files = |_: &Path| Ok(vec![RomFsFile::new("a.txt", b"a"), RomFsFile::new("d/b.txt", b"b")]);
assert!(build_romfs(Path::new("/assets"), two_files).is_ok());
```

2. **Take the ambient value; do not reach for it**
   The clock, the environment, and the working directory are inputs. Read them at the entrypoint and pass them
   down.

```rust
// ❌ Bad — `Instant::now` and the env var are read deep in the call graph. Testing "discovery
// gives up after the deadline" now requires the test to actually sleep, so it either takes
// thirty seconds or is written to sleep one millisecond and is flaky on a loaded CI box.
pub async fn discover(sock: &UdpSocket) -> Result<ConsoleAddr, DiscoverError> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let retries: u32 = std::env::var("NX_DISCOVERY_RETRIES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // ...
}
```

```rust
// ✅ Good — the policy and the clock arrive as arguments; the entrypoint reads the environment once.
pub struct RetryPolicy {
    pub retries: u32,
    pub deadline: Instant,
}

pub async fn discover(
    sock: &UdpSocket,
    policy: RetryPolicy,
    now: impl Fn() -> Instant,
) -> Result<ConsoleAddr, DiscoverError> {
    // tests drive `now` forward by hand and assert the give-up behavior without sleeping
}
```

## Why It Matters

Without injection, a unit's dependencies are invisible: nothing in `build_romfs(root)` says it touches the
disk. With injection, the signature _is_ the dependency list, and a reviewer sees a function's whole blast
radius without reading its body.

It is also the only seam Rust gives you. There is no runtime patching here: a `TcpStream::connect(..)` inside a
function body cannot be replaced by a test, at any price. Either the collaborator is in the signature or the
code is reachable only through an end-to-end test — and an end-to-end test here needs a console listening on
the LAN, so it runs by hand rather than in the edit loop, and the awkward cases end up uncovered.

Injection is what keeps the dependency graph acyclic, too: `nx-object`'s builders never depend on the CLI's
`ui` module, because the sink a command reports through arrives as a value the binary composes and passes in.

## Pragmatism Caveat

Do not inject for the sake of it. A transfer may open its own `TcpStream` from the address it was handed: the
stream's lifetime is the transfer's lifetime, no other implementation exists, and a test injects the address of
a local listener instead — the seam is the address, not the stream. Injecting the value one level up (an
address, a spec, a path) is very often better than injecting the object.

Do not reach for `Arc<dyn Trait>` where a generic parameter fits, and do not introduce a trait with exactly one
implementation and no test double: that is indirection bought with nothing.

When an I/O dependency is deliberately constructed inside, the seam that replaces it must exist and be named in
a comment. An I/O dependency with no seam at all is always wrong.

## Checklist

Before committing code, verify:

- [ ] Every I/O collaborator is a parameter, a constructor argument, or a generic parameter — not constructed
      inline
- [ ] Effects a test needs to control are parameters (a scanner closure, a stream, a ui sink, a clock), with the
      real implementation supplied by the caller
- [ ] `std::env`, the current directory, and the clock are read at entrypoints and passed down, not read deep in
      the call graph
- [ ] The unit can be exercised with a hand-written fake or in-memory bytes, without a console on the network or
      a temp directory
- [ ] Generic parameters are used for fixed collaborators; trait objects only where the set is runtime-assembled
- [ ] Where a dependency is constructed internally, the injectable seam (address, spec, path) is documented
- [ ] No trait exists solely to wrap a single implementation that nothing substitutes

## References

- [principle-law-of-demeter](principle-law-of-demeter.md) - Related: Injecting the exact value needed removes
  the reason to navigate an object graph
- [principle-single-responsibility](principle-single-responsibility.md) - Related: A unit with one responsibility
  has few enough collaborators to inject them all
- [principle-open-closed](principle-open-closed.md) - Related: The injection seam and the extension point are
  the same trait

## External References

- [Inversion of Control (Kent C. Dodds)](https://kentcdodds.com/blog/inversion-of-control)
- [Inversion of Control Containers and the Dependency Injection pattern (Martin Fowler)](https://martinfowler.com/articles/injection.html)
- [Beginner's Guide to Inversion of Control (HackerNoon)](https://hackernoon.com/beginners-guide-to-inversion-of-control)
