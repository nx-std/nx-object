---
name: "test-organization"
description: "Unit, integration and e2e tiers with the it_* convention and how each suite is run. Load when deciding test type or placement"
type: core
scope: "global"
---

# Test Organization

**MANDATORY for choosing the tier and variant of any new test**

## Three Tiers

| Tier | Dependencies | Speed | Purpose | Selected by |
|------|-------------|-------|---------|-------------|
| **Unit** | None | Milliseconds | Pure logic and byte-level transforms | No `it_` in the path |
| **Integration** | Filesystem, process, network | Seconds | Components with real dependencies | `it_` module or file |
| **E2E** | Filesystem, process, network | Seconds | Cross-crate end-to-end workflows | `it_` file in `tests/` |

Each tier has a role the others cannot fill: unit tests cannot verify that a packed image survives a round trip through the filesystem, integration tests cannot verify cross-crate workflows, and e2e tests are too slow and broad for isolated logic. Start with unit tests for pure logic, use integration tests for components with external dependencies, and use e2e tests for cross-crate end-to-end workflows.

For how to **run** tests (justfile tasks, per-crate commands), see the `/code-test` skill.

## In-tree vs Public API Variants

The unit and integration tiers each split into two variants based on **where** the test lives and **what** it can access:

| Variant | Location | API Access | Distinguishing Convention |
|---------|----------|------------|--------------------------|
| **In-tree** | `src/` (`#[cfg(test)]` modules) | Internal + public APIs | Unit: `tests::` (no `it_*`), Integration: `tests::it_*` |
| **Public API** | `<crate>/tests/` directory | Public API only | Unit: no `it_*` prefix, Integration: `it_*` prefix |

The `it_*` prefix is the **sole mechanism** that distinguishes integration tests from unit tests in both locations. Tests without `it_*` are unit tests; tests with `it_*` are integration tests. That is what lets a run select one tier: `-E 'test(/::it_/)'` for the integration tier, `--skip 'tests::it_'` for everything else.

In-tree tests reach internal APIs, which is what makes it possible to test non-public query functions, internal helpers, and error paths in internal components. Public API tests verify the external contract: that the exported interface is ergonomic and correct, that advertised workflows work, and that errors propagate through it. Location determines API access; the `it_*` prefix determines whether external dependencies are involved.

## Unit Tests

Unit tests must have **no external dependencies** and execute in **milliseconds**. They validate pure logic, byte-level transformations, and error handling without touching the filesystem, spawning a process, or opening a socket.

- **No external dependencies**: no spawned processes, no network calls, no filesystem operations
- **Performance**: must complete in milliseconds
- **Reliability**: 100% deterministic, no flakiness
- **No `it_*` prefix**: test functions and modules must NOT use the `it_*` naming convention

Variants:

- **In-tree**: `src/` files inside `#[cfg(test)] mod tests { ... }`; internal and public APIs; selected by `kind(lib)` excluding `test(/::it_/)`
- **Public API**: `<crate>/tests/` files without an `it_*` prefix; public API only (separate crate); selected by `kind(test)` excluding `test(/::it_/)`

What to cover: input validation logic (entry path validation, argument checking, magic and size checks), format rule enforcement (segment bounds, alignment, header invariants), data transformation (parsing, encoding, type conversion), error condition handling (truncated buffers, invalid inputs, edge conditions), and pure computational functions (layout planning, compression, checksums).

```rust
// ✅ Good — in-tree unit test: pure logic, no it_ prefix, so it stays in the unit tier
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_entry_path_with_valid_input_succeeds() {
        //* Given
        let valid_path = "romfs/config.json";

        //* When
        let result = validate_entry_path(valid_path);

        //* Then
        assert!(result.is_ok(), "validation should succeed with valid input");
        assert_eq!(result.expect("should return valid value"), valid_path);
    }
}

// ✅ Good — public API unit test in tests/api_validation.rs: no it_ prefix, exported items only
use nx_object::TitleId;

#[test]
fn parse_title_id_from_string_succeeds() {
    //* Given
    let input = "0100000000001000";

    //* When
    let result = input.parse::<TitleId>();

    //* Then
    let title_id = result.expect("should parse valid title ID");
    assert_eq!(title_id.to_string(), input);
}
```

## Integration Tests

Integration tests verify that components work correctly with **external dependencies** such as the filesystem, a spawned `cargo` invocation, or a console reachable over the network.

- **External dependencies**: real files and processes (for example a `tempfile::tempdir` fixture tree, or a spawned build)
- **Mandatory `it_*` prefix on the parent module**: integration tests must live inside an `it_*`-prefixed module or file for filtering
- **Flakiness risk**: may fail due to external dependency issues (a busy port, a slow disk, a missing toolchain)
- **Performance**: seconds, not milliseconds

Variants:

- **In-tree**: `src/` files inside `tests::it_*` submodules, either as separate files `src/<module>/tests/it_*.rs` or inline `mod it_*` submodules; internal and public APIs; selected by `kind(lib)` with `test(/::it_/)`
- **Public API**: `<crate>/tests/` files with an `it_*` prefix; public API only (separate crate); selected by `kind(test)` with `test(/::it_/)`

What to cover: filesystem operations (writing an image, reading it back, permissions), partial-failure behavior (an aborted write leaves no half-packed artifact), error handling with external systems (a refused connection, a missing file, a timeout), resource management (temp directory cleanup, socket lifecycle), and format compatibility (an artifact written by the builder parses back through the reader).

```rust
// ✅ Good — in-tree integration test: disk-backed work sits under an it_ module
#[cfg(test)]
mod tests {
    use super::*;

    mod it_image {
        use super::*;

        #[test]
        fn write_image_with_populated_dir_succeeds() {
            //* Given
            let dir = tempfile::tempdir().expect("temp dir should be created");
            let out = dir.path().join("image.romfs");

            //* When
            let result = write_image(dir.path(), &out);

            //* Then
            assert!(result.is_ok(), "image write should succeed");
            let written = std::fs::metadata(&out)
                .expect("should stat the written image")
                .len();
            assert!(written > 0, "the image should not be empty");
        }
    }
}
```

```rust
// ✅ Good — public API integration test in tests/it_bundle.rs: it_ prefix, exported items only
use nx_object::{read::Nro, write::NroBuilder};

#[test]
fn build_and_parse_nro_round_trips() {
    //* Given
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let elf = fixture_elf(dir.path());

    //* When
    let result = NroBuilder::new().elf(&elf).build();

    //* Then
    assert!(result.is_ok(), "the NRO build should succeed");
    let bytes = result.expect("should return the packed NRO");
    let nro = Nro::try_from_bytes(&bytes).expect("the packed NRO should parse back");
    assert_eq!(nro.segments().len(), 3);
}
```

## E2E Tests

E2E tests cover a whole format round trip: a linked ELF turned into an NRO and parsed back, or a directory tree built into a RomFS image and walked entry by entry. They need a real environment (a fixture binary, a temp directory tree), and live in `tests/` under the `it_` prefix like any other integration file.

What to cover: whole-pipeline workflows (an ELF becoming segments becoming a packed NRO), layer integration (a `write` builder's output parsed by the matching `read` type), and complete user scenarios (a directory on disk through to an image the console mounts).

## Running the Suites

There is one runner and one scope:

- `just test` runs the crate's tests, and takes extra flags: `just test --all-features -- <filter>`.

It prefers `cargo nextest run` and fall back to `cargo test` when nextest is not installed. Neither excludes the `it_` tier: the prefix exists so a developer can select or skip it explicitly — `cargo nextest run -E 'test(/::it_/)'` for the integration tier, `cargo test -- --skip 'tests::it_'` for everything else.

A test that needs something the machine may not have — a console on the network, a toolchain component — states that in its name and skips cleanly when the dependency is absent, rather than failing a suite that never promised it.

## Checklist

When deciding which test tier and variant to use:

- [ ] Does the function have zero external dependencies? → **Unit test**
- [ ] Does the function need the filesystem, a process, or the network? → **Integration test** (use `it_*` prefix)
- [ ] Does the test span a whole format round trip end-to-end? → **E2E test** (in `tests/`)
- [ ] Does the test need access to internal APIs? → **In-tree** variant (in `src/`)
- [ ] Should the test only use the public API? → **Public API** variant (in `<crate>/tests/`)
- [ ] Is the test fast (milliseconds)? → Unit test
- [ ] Is the test slow (seconds) due to external dependencies? → Integration test with `it_*` prefix
- [ ] Does the test need a dependency the machine may not have? → It says so in its name and skips cleanly when absent

## References

- [test-files](test-files.md) - Related: Where test modules and files live in the directory structure
- [test-functions](test-functions.md) - Related: Naming, Given-When-Then structure, and assertions inside a test function
