---
name: "rust-errors-handling"
description: "Propagating and recovering errors: unwrap/expect ban, pattern matching, justifying a discarded error. Load when handling a Result or Option"
type: core
scope: "global"
---

# Rust Error Handling Patterns

**MANDATORY for ALL Rust code in this workspace**

## 1. Never `.unwrap()` or `.expect()` in Production

**ABSOLUTELY CRITICAL - ZERO TOLERANCE POLICY**

**DO NOT** use `.unwrap()` or `.expect()` in a production code path unless you can prove the operation cannot
fail. A panic kills the process: destructors and cleanup are skipped, an in-flight transaction is abandoned
mid-write, a dependent component in a distributed system fails with it, and the panic message carries less
about the failure than the error it discarded.

```rust
// ❌ Bad — a missing file or one malformed byte takes down the build, and the panic
// reports less than the io::Error it threw away
pub fn load_npdm(path: &Path) -> NpdmSpec {
    let contents = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&contents).unwrap()
}

// ❌ Bad — a message does not make the panic acceptable; this still aborts the process
pub fn load_npdm(path: &Path) -> NpdmSpec {
    let contents = std::fs::read_to_string(path).expect("failed to read npdm");
    serde_json::from_str(&contents).expect("failed to parse npdm")
}

// ✅ Good — the caller decides what a missing or malformed descriptor means
pub fn load_npdm(path: &Path) -> Result<NpdmSpec, LoadNpdmError> {
    let contents = std::fs::read_to_string(path).map_err(LoadNpdmError::ReadFailed)?;
    let spec = serde_json::from_str(&contents).map_err(LoadNpdmError::ParseFailed)?;
    Ok(spec)
}
```

**Code review red flag:** any `.unwrap()` or `.expect()` in a production path is rejected unless a code
invariant makes the failure impossible, and that invariant is written down where it is assumed:

1. **Proof of safety** - a logical analysis or type-system guarantee that the operation cannot fail
2. **`// SAFETY:` comment on the full statement** - placed immediately above the statement that holds the
   `.unwrap()`/`.expect()` (not on the message string), naming the invariant that makes the panic impossible,
   so a reviewer can check the claim and a later change to the invariant has a searchable site to re-examine

Do **not** add a `# Panics` rustdoc section for such a call. The invariant makes it unreachable, so a `# Panics`
section would document a failure that cannot occur and mislead the caller. `# Panics` is for a panic that a
caller can actually trigger ([rust-docs-rustdoc](rust-docs-rustdoc.md)); a provably-unreachable `.expect()` is
not one. If the failure *can* happen, the call is not provably safe: return a `Result` instead.

**Even when it is provably safe, prefer refactoring to eliminate it entirely.**

```rust
// ✅ Good — the invariant sits above the statement, not buried in the expect message, and no
// # Panics section is written because the lock is private and never held across a panic.
// SAFETY: `self.state` is only locked here and in `record`, both of which drop the guard before
// returning, so the mutex can never be poisoned.
let mut state = self.state.lock().expect("state mutex poisoned");
```

A genuinely fallible construction is the case that fits neither. Binding the discovery socket can fail when
another process already holds the port, and no code invariant rules that out. A `// SAFETY:` comment would
assert something the code does not guarantee, and a `# Panics` section would document a panic the caller can
neither provoke through its arguments nor prevent. The honest form is a `Result` the caller propagates, built
at the composition root where a constructor already returns one.

```rust
// ❌ Bad — the failure is real (a port already in use), so the SAFETY comment overclaims and the
// `.expect()` turns an environment problem into a process abort.
// SAFETY: the socket always binds.
let socket = UdpSocket::bind(RECEIVE_ADDR).await.expect("discovery socket binds");

// ✅ Good — the constructor is fallible, so the caller decides what a taken port means.
async fn with_discovery_socket() -> Result<Self, io::Error> {
    let socket = UdpSocket::bind(RECEIVE_ADDR).await?;
    Ok(Self::new(socket))
}
```

## 2. Prefer Pattern Matching

**ALWAYS** handle `Result` and `Option` by matching. The type system is your ally - use it.

### `let-else` for an Early Return

```rust
// ✅ Good — the failure exits immediately and `assets` is a value, not an Option, below
pub fn icon(&self) -> Result<&'a [u8], IconError> {
    let Some(assets) = self.asset_header else {
        return Err(IconError::NoAssetSection);
    };

    self.section_bytes(assets.icon)
}
```

### `match` for Multiple Cases

```rust
// ✅ Good — each failure maps to the exit it deserves, and a new SendNroError variant
// breaks the build instead of silently taking the catch-all
pub fn report(result: Result<(), SendNroError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(SendNroError::NoServerFound) => ui::hint_console_not_listening(),
        Err(SendNroError::InsufficientSpace) => ui::hint_free_space(),
        Err(err) => ui::report(err),
    }
}
```

### `if let` for a Single Case

```rust
// ✅ Good — the error is reported where it happens and the caller is not burdened
pub fn maybe_log_error(result: Result<(), Error>) {
    if let Err(err) = result {
        tracing::error!(
            error = %err,
            error_source = logging::error_source(&err),
            "operation failed"
        );
    }
}
```

### Combinators for Transformation Chains

```rust
// ✅ Good — the absent case produces a value instead of a branch
pub fn entry_name(&self, id: EntryId) -> String {
    self.entries
        .get(&id)
        .map(|entry| entry.name.clone())
        .unwrap_or_else(|| format!("unnamed-{id}"))
}

// ✅ Good — `ok_or` turns a missing value into the error that names it
pub fn require_npdm(spec: Option<NpdmSpec>) -> Result<NpdmSpec, Error> {
    spec.ok_or(Error::NpdmMissing)
}
```

## 3. A Discarded Error Carries a Justification

Dropping a `Result` — `let _ = fallible();`, `.ok()`, an `Err(_)` arm that does not propagate — is a decision,
and it is invisible unless it is written down. The next reader cannot tell a considered discard from a bug,
so every one carries a comment naming **what would break if the error escaped**.

```rust
// ❌ Bad — a silent discard. Nothing distinguishes this from a forgotten `?`,
// and the failure it swallows never appears anywhere.
let _ = self.stdout.write_all(line).await;
```

```rust
// ✅ Good — the comment names what the discard protects, so the cost of the lost
// error can be weighed.
// A closed pipe means the user redirected our output and the reader went away; the
// transfer outlives the forwarded stdio on purpose, and failing here would abort a
// deployment that has already landed on the console.
let _ = self.stdout.write_all(line).await;
```

Discarding is not a way to avoid handling an error. If the failure matters to the caller, propagate it; if it
matters to an operator, log it ([logging-errors](logging-errors.md)). The comment is only for the case where
losing it is genuinely correct. What the comment must say is governed by
[rust-docs-comments](rust-docs-comments.md).

## 4. Test Code Exception

**EXCEPTION**: `.expect()` with a descriptive message is **acceptable and recommended in test code**. A test
should fail loudly when a precondition is not met, and the message names which one.

Message format: `"<operation> should <expected behavior>"`. Never use `.unwrap()`, even in tests.

```rust
// ✅ Good — a failure names the step that broke without opening the test
#[test]
fn it_packs_an_nro_with_an_asset_section() {
    //* Given
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let elf = fixture_elf(&dir);

    //* When
    let nro = pack_nro(&elf, Some(fixture_assets()))
        .expect("packing should succeed for a well-formed ELF");

    //* Then
    let parsed = Nro::try_from_bytes(&nro).expect("the packed NRO should parse back");

    assert!(parsed.has_assets(), "the asset section should survive the round trip");
}

// ❌ Bad — the panic points at a line number and nothing else, so a red CI run does not
// say whether packing, parsing, or the assertion's precondition failed
#[test]
fn it_packs_an_nro() {
    let dir = tempfile::tempdir().unwrap();
    let nro = pack_nro(&fixture_elf(&dir), Some(fixture_assets())).unwrap();
    let parsed = Nro::try_from_bytes(&nro).unwrap();
    assert!(parsed.has_assets());
}
```

## Checklist

Before committing Rust code, verify:

- [ ] **ZERO `.unwrap()` calls in production code paths**
- [ ] **ZERO `.expect()` calls in production code (except provably safe with documentation)**
- [ ] Pattern matching used for all `Result` and `Option` handling
- [ ] `let-else` used for early returns from `Option`
- [ ] `match` used for explicit multi-branch handling
- [ ] `if let` used for single-case handling
- [ ] Combinators (`.map()`, `.ok_or()`, `.and_then()`) used appropriately
- [ ] Every discarded `Result` (`let _ =`, `.ok()`, a non-propagating `Err(_)` arm) carries a comment naming
      what would break if the error escaped
- [ ] Test code uses `.expect()` with descriptive messages (NOT `.unwrap()`)
- [ ] Every unwrap/expect in production code carries a `// SAFETY:` comment on its full statement naming the
      invariant that makes the panic impossible, plus a logical or type-system proof of safety
- [ ] No `# Panics` section is added for a provably-unreachable unwrap/expect; the `// SAFETY:` comment is what
      documents it
- [ ] Functions return `Result<T, E>` for all fallible operations
- [ ] Error types provide rich context (see [rust-errors-reporting](rust-errors-reporting.md))
- [ ] No panic-inducing code without documentation and proof

## References

- [rust-errors-reporting](rust-errors-reporting.md) - Related: Declaring the error types propagated here
- [rust-docs-rustdoc](rust-docs-rustdoc.md) - Related: Owns the `# Panics` section, and when a provably-safe
  unwrap/expect omits it
