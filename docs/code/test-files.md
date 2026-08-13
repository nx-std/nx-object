---
name: "test-files"
description: "Test file placement, cfg(test) modules, it_* naming, in-tree vs tests/ directory. Load when creating test files or organizing test modules"
type: core
scope: "global"
---

# Test Files

**MANDATORY for placing test modules and test files in any crate**

## Canonical Layout

```
<crate-root>/
  src/
    module.rs              # Source + #[cfg(test)] mod tests { ... }
    module/
      tests/
        validation.rs      # Unit tests (NO it_ prefix)
        it_image.rs        # In-tree integration tests (it_ prefix)
  tests/
    it_bundle.rs           # Public API integration tests (it_ prefix)
```

The `it_` prefix is the **sole mechanism** that distinguishes integration tests (external dependencies: the filesystem, a spawned `cargo`, a console on the network) from unit tests (no external dependencies, milliseconds).

## Unit Test Placement

Unit tests have **no external dependencies** and execute in **milliseconds**. They validate pure business logic, data transformations, and error handling.

### Co-located Tests

Tests live in the same file as the code, inside a `#[cfg(test)]` module. Use this when the module has few tests (under ~50 lines), the tests are simple, and no complex fixtures are needed.

```rust
// ✅ Good — tests sit next to the code they cover, so they get updated when it changes
fn validate_entry_path(path: &str) -> Result<String, ValidationError> {
    if path.is_empty() {
        return Err(ValidationError::EmptyPath);
    }
    if path.starts_with('/') {
        return Err(ValidationError::Absolute);
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod validation {
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
}
```

`#[cfg(test)]` keeps test code out of production binaries, so co-location costs nothing at runtime.

### In-tree `tests/` Directory

Extract tests to `src/<module>/tests/` when the suite grows past ~50 lines, needs complex fixtures or setup, or spans several files for one module.

Unit test files and modules there **MUST NOT** start with `it_`.

```rust
// ✅ Good — src/write/tests/validation.rs carries no it_ prefix, so it stays in the unit tier
use crate::write::romfs::*;

mod unit_validation {
    use super::*;

    #[test]
    fn validate_entry_path_with_empty_input_fails() {
        //* Given
        let empty_path = "";

        //* When
        let result = validate_entry_path(empty_path);

        //* Then
        assert!(result.is_err(), "validation should fail with empty input");
        let error = result.expect_err("should return validation error");
        assert!(matches!(error, ValidationError::EmptyPath),
            "Expected EmptyPath error, got {:?}", error);
    }
}

mod header_encoding {
    use super::*;

    #[test]
    fn encode_header_with_valid_spec_succeeds() { /* ... */ }
}
```

## In-tree Integration Test Placement

In-tree integration tests cover **internal functionality** not exposed through the crate's public API, and they have **external dependencies** (the filesystem, a spawned process, a console on the network).

Their module or file name **MUST** start with `it_`.

### Inline Integration Submodule

Use when the tests are closely tied to the implementation and few in number.

```rust
// ✅ Good — integration tests sit beside the code they cover, isolated in an it_ submodule
pub fn write_image(dir: &Path, out: &Path) -> Result<u64, WriteImageError> {
    /* ... */
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests for pure functions here...

    mod it_image {  // ✅ Good — the it_ prefix keeps disk-backed tests out of the unit tier
        use super::*;

        #[test]
        fn write_image_with_populated_dir_succeeds() {
            //* Given
            let dir = tempfile::tempdir().expect("temp dir should be created");
            let out = dir.path().join("image.romfs");
            let seed_result = seed_assets(dir.path());
            assert!(seed_result.is_ok(), "asset seeding should succeed");

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

### External Integration Test File

Use for large integration suites, complex setup that warrants a dedicated file, or several integration files for one module.

```rust
// ✅ Good — src/write/tests/it_image.rs uses the it_ prefix required for filtering
use crate::write::romfs::*;

#[test]
fn write_image_with_populated_dir_succeeds() {
    //* Given
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let out = dir.path().join("image.romfs");

    //* When
    let result = write_image(dir.path(), &out);

    //* Then
    assert!(result.is_ok(), "image write should succeed");
}
```

## Public API Integration Test Placement

Public API integration tests verify **end-to-end functionality** through the **crate's public API only**, using Rust's standard `<crate-root>/tests/` directory (outside `src/`). Each file compiles as a separate crate, so no internal API is reachable. These tests may use external dependencies.

Files there **MUST** be named `it_*`.

```rust
// ✅ Good — tests/it_bundle.rs exercises only exported items, so refactoring internals cannot break it
use nx_object::{read::Nro, write::NroBuilder};

#[test]
fn build_and_parse_nro_round_trips() {
    //* Given
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let elf = fixture_elf(dir.path());
    let build_result = NroBuilder::new().elf(&elf).build();
    assert!(build_result.is_ok(), "the NRO build should succeed");

    //* When
    let bytes = build_result.expect("should return the packed NRO");
    let parse_result = Nro::try_from_bytes(&bytes);

    //* Then
    assert!(parse_result.is_ok(), "the packed NRO should parse back");
    let nro = parse_result.expect("should return the parsed NRO");
    assert_eq!(nro.segments().len(), 3);
    assert!(!nro.has_assets(), "no asset section was requested");
}
```

## The it_ Naming Convention

| Test Type | Location | Naming Rule | Example |
|-----------|----------|-------------|---------|
| **Unit** (no external deps) | `#[cfg(test)] mod tests` | **NO** `it_` prefix | `mod validation` |
| **Unit** (no external deps) | `src/*/tests/*.rs` | **NO** `it_` prefix | `tests/validation.rs` |
| **In-tree Integration** | `#[cfg(test)] mod tests` | **YES** `it_` prefix | `mod it_image` |
| **In-tree Integration** | `src/*/tests/*.rs` | **YES** `it_` prefix | `tests/it_image.rs` |
| **Public API Integration** | `tests/*.rs` | **YES** `it_` prefix | `tests/it_bundle.rs` |

The prefix is what makes test selection possible:

- Nextest selection: `cargo nextest run -E 'test(/::it_/)'` runs the integration tier, and `-E 'not test(/::it_/)'` runs the unit tier
- Targeted execution: `cargo test tests::it_` runs the integration tests, and `-- --skip 'tests::it_'` runs
  everything else. Cargo's filter is a substring match on the full test path, so `cargo test tests::` selects
  the `it_` tests too — use the skip form to exclude them
- Test output carries the module path, so failures name the tier

**Violating this convention breaks test filtering**, which shows up as slow local runs, CI failures from a missing toolchain or console, and unexplained local test failures.

## Module Structure Within cfg(test)

For fewer than ~10 tests, a flat list of test functions inside `#[cfg(test)] mod tests` is sufficient. Once a module reaches 10+ tests, group them into nested `mod` blocks by concern so failure paths such as `tests::validation::validate_input_with_empty_string_fails` name the broken area, and so each concern can hold its own test utilities.

```rust
// ✅ Good — concerns are grouped, so a failing path names the area that broke
#[cfg(test)]
mod tests {
    use super::*;

    mod constructors {
        use super::*;

        #[test]
        fn new_with_valid_config_succeeds() { /* ... */ }

        #[test]
        fn new_with_invalid_config_fails() { /* ... */ }
    }

    mod validation {
        use super::*;

        #[test]
        fn validate_input_with_valid_data_succeeds() { /* ... */ }

        #[test]
        fn validate_input_with_invalid_data_fails() { /* ... */ }
    }

    mod it_image_operations {  // ✅ Good — external-dependency group carries the it_ prefix
        use super::*;

        #[test]
        fn image_operations_work_end_to_end() { /* ... */ }
    }
}
```

## Progressive Test Complexity

Order tests within a module from simple to complex, so the right test is easy to find when debugging:

1. **Basic functionality** — happy path with minimal setup
2. **With configuration** — custom options and parameters
3. **Error scenarios** — invalid inputs, boundary cases
4. **External dependencies** — filesystem, spawned process, network
5. **Full integration** — complete workflows, multiple artifacts

```rust
// ✅ Good — simple cases come first, so the first failure is the cheapest one to debug
#[cfg(test)]
mod tests {
    use super::*;

    mod feature_progression {
        use super::*;

        // 1. Basic functionality
        #[test]
        fn validate_input_with_defaults_succeeds() { /* ... */ }

        // 2. With configuration
        #[test]
        fn validate_input_with_custom_config_succeeds() { /* ... */ }

        // 3. Error scenarios
        #[test]
        fn validate_input_with_empty_string_fails() { /* ... */ }
    }

    mod it_feature_progression {
        use super::*;

        // 4. External dependencies
        #[test]
        fn write_image_with_populated_dir_succeeds() { /* ... */ }

        // 5. Full integration
        #[test]
        fn build_and_bundle_workflow_succeeds() { /* ... */ }
    }
}
```

## File Naming Rules

| Test Type | File Location | Filename Pattern | Example |
|-----------|---------------|------------------|---------|
| **Co-located unit** | Same as source | `*.rs` with `#[cfg(test)]` | `src/write/romfs.rs` |
| **In-tree unit** | `src/*/tests/` | No `it_` prefix | `src/write/tests/validation.rs` |
| **In-tree integration** | `src/*/tests/` | `it_*.rs` prefix | `src/write/tests/it_image.rs` |
| **Public API integration** | `tests/` (crate root) | `it_*.rs` prefix | `tests/it_bundle.rs` |

The `it_` prefix on filenames in `src/*/tests/` and `tests/` is **MANDATORY** for integration tests.

## Checklist

Before creating or moving test files, verify:

- [ ] Unit tests (no external deps) are co-located or in `src/*/tests/` without `it_` prefix
- [ ] In-tree integration tests (with external deps) use `it_` prefix in module or filename
- [ ] Public API integration tests are in `tests/` directory with `it_*.rs` naming
- [ ] All tests use `#[cfg(test)]` module structure when co-located
- [ ] Module names accurately reflect whether tests have external dependencies
- [ ] Test file location matches test type (unit vs integration)

## References

- [test-functions](test-functions.md) - Related: Naming, Given-When-Then structure, and assertions inside a test function
- [test-organization](test-organization.md) - Related: Test tier selection (unit, integration, e2e) and how the suites are run
