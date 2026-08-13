---
name: "test-functions"
description: "Test naming conventions, function structure, Given-When-Then, async tests, assertions, forbidden patterns. Load when writing or reviewing test functions"
type: core
scope: "global"
---

# Test Functions - Naming and Structure

**MANDATORY patterns for writing individual test functions in Rust**

## Test Naming Conventions

Naming is the first decision when writing a test. Every test function uses the format:

`<function_name>_<scenario>_<expected_outcome>()`

- **function_name**: the exact name of the function being tested
- **scenario**: the specific input condition, state, or situation
- **expected_outcome**: what should happen (`succeeds`, `fails`, `returns_none`, ...)

A name in this form answers what is tested, under what conditions, and what should happen, so a CI failure is legible without opening the test body.

```rust
// ✅ Good — each name states the scenario and the outcome, so a failing CI line is self-explanatory
#[test]
fn try_from_bytes_with_valid_nro_succeeds() { /* ... */ }

#[test]
fn try_from_bytes_with_truncated_segment_fails() { /* ... */ }

#[test]
fn asset_header_with_no_asset_section_returns_none() { /* ... */ }

#[tokio::test]
async fn discover_with_no_console_listening_returns_none() { /* ... */ }

#[test]
fn validate_entry_path_with_max_length_succeeds() { /* ... */ }
```

```rust
// ❌ Bad — vague names force a reader to open the body to learn what broke
#[test]
fn test_parse() { /* ... */ }

#[test]
fn parse_works() { /* ... */ }

// ❌ Bad — "test" in the name is redundant; #[test] already says it
#[test]
fn test_try_from_bytes_with_valid_nro_succeeds() { /* ... */ }

#[test]
fn validate_entry_path_test_returns_error() { /* ... */ }

// ❌ Bad — missing scenario: which input made it succeed?
#[test]
fn try_from_bytes_succeeds() { /* ... */ }

// ❌ Bad — missing expected outcome: succeeds or fails?
#[test]
fn try_from_bytes_with_valid_nro() { /* ... */ }

// ❌ Bad — two functions under test violates single responsibility; split into two tests
#[test]
fn build_and_parse_nro_succeeds() { /* ... */ }
```

### Naming by Test Type

Unit tests name input conditions and format rules; integration tests name the state of the external thing they touch; end-to-end tests name workflows.

```rust
// ✅ Good — unit: the scenario is an input condition
fn parse_npdm_with_malformed_json_fails() {}

// ✅ Good — integration: the scenario is the state on disk
fn write_image_with_empty_dir_produces_empty_image() {}

// ✅ Good — end-to-end: the scenario is a workflow
fn build_and_deploy_nro_workflow_succeeds() {}
```

### Length and Clarity

Be descriptive but concise, use domain terminology consistently, avoid abbreviations that are not well established in the domain, and stay near ~60 characters where possible, prioritizing clarity over the limit.

```rust
// ✅ Good — descriptive and still readable at a glance
fn send_nro_with_insufficient_console_space_returns_error() {}

// 🔶 Acceptable — over-long, but only where the extra words are needed for clarity
fn try_from_bytes_with_asset_header_past_the_declared_size_returns_none() {}

// ❌ Bad — abbreviations that no reader can decode
fn tfb_inv_hdr_fails() {}
```

## Testing Framework Selection

Use standard `#[test]` for synchronous functions and `#[tokio::test]` for async functions.

```rust
// ✅ Good — sync function under test, so a plain #[test] is enough
#[test]
fn plan_segments_with_page_aligned_elf_returns_three_bounds() {
    //* Given
    let elf = fixture_elf();

    //* When
    let result = plan_segments(&elf);

    //* Then
    assert_eq!(result.len(), 3);
}

// ✅ Good — async function under test needs a runtime
#[tokio::test]
async fn discover_with_no_console_listening_returns_none() {
    //* Given
    let timeout = Duration::from_millis(50);

    //* When
    let result = discover(timeout, 1).await;

    //* Then
    assert!(result.expect("discovery should not error").is_none());
}
```

## Given-When-Then Structure (Mandatory)

Every test follows the Given-When-Then pattern with **MANDATORY** `//* Given`, `//* When`, and `//* Then` marker comments. The markers are keyword-only: no trailing text on the marker line; any prose goes on the next line as an ordinary `//` comment.

| Marker | Required | Purpose | Content |
|---|---|---|---|
| `//* Given` | Optional (omit when there is no setup) | Preconditions, test data, fixtures, system state | Variable declarations, temp directory setup, fixture construction |
| `//* When` | Required | Execute **exactly one** function under test | **Only** the single call being tested |
| `//* Then` | Required | Assert outcomes and side effects | **Only** assertions and assertion helpers such as `.expect()` used to extract a value |

More than one call in `//* When` means the test scope is too broad and failure attribution is impossible. Business logic in `//* Then` obscures what is being verified: if a value must be transformed before asserting, the transformation belongs in `//* Given`, or the test is verifying two things and should be split.

```rust
// ✅ Good — one call under test, and the Then section only asserts
#[test]
fn write_image_with_populated_dir_succeeds() {
    //* Given
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let out = dir.path().join("image.romfs");
    let expected_entries = seed_assets(dir.path());

    //* When
    let result = write_image(dir.path(), &out);

    //* Then
    assert!(result.is_ok(), "image write should succeed with a populated dir");
    let written = result.expect("should return the written byte count");
    assert!(written > 0, "the written byte count should be positive");
    let image = RomFs::try_from_bytes(&std::fs::read(&out).expect("should read the image"))
        .expect("the written image should parse back");
    assert_eq!(image.root_dir().entries().count(), expected_entries);
}

// ✅ Good — no setup needed, so Given is omitted
#[test]
fn get_default_with_uninitialized_state_fails() {
    //* When
    let result = get_default();

    //* Then
    assert!(result.is_err(), "validation should fail for uninitialized state");
    let error = result.expect_err("should return validation error");
    assert!(matches!(error, ValidationError::EmptyInput),
        "Expected EmptyInput error, got {:?}", error);
}
```

```rust
// ❌ Bad — no markers, so the reader cannot tell setup from action from assertion
#[test]
fn validate_input_with_valid_data_succeeds() {
    let input = "test";
    let result = validate_input(input);
    assert!(result.is_ok());
}

// ❌ Bad — two functions in When, so a failure names neither of them
#[test]
fn add_file_with_existing_entry_succeeds() {
    //* Given
    let mut builder = RomFsBuilder::new();

    //* When
    let path = validate_entry_path("romfs/config.json").expect("path should be valid");
    let result = builder.add_file(path, b"{}".to_vec());

    //* Then
    assert!(result.is_ok());
}

// ❌ Bad — business logic in Then hides which behavior is actually under test
#[test]
fn asset_header_with_asset_section_returns_header() {
    //* Given
    let bytes = fixture_nro_with_assets();

    //* When
    let result = Nro::try_from_bytes(&bytes);

    //* Then
    assert!(result.is_ok());
    let nro = result.expect("should parse the NRO");
    let icon = nro.icon().expect("should have an icon");
    let expected_len = icon.len().next_power_of_two();
    assert_eq!(padded_len(icon), expected_len);
}
```

## Forbidden Patterns

### Never Use `unwrap()` in Tests

A panicking `unwrap()` reports only "called `Result::unwrap()` on an `Err` value". `.expect("...")` turns the same panic into an actionable message naming what was expected and what happened.

```rust
// ❌ Bad — panics with no context about what was expected
let result = risky_operation(input).await.unwrap();

// ✅ Good — the panic message names the expectation and prints the actual error
let result = risky_operation(input).await
    .expect("risky_operation should succeed with valid input");
```

### Never Test Multiple Functions in One Test

One test exercises exactly one function, as required by the `//* When` rule above. Two calls under test means a failure names neither of them; split the test in two.

## Assertion Patterns

Every assertion carries a descriptive failure message.

```rust
// ✅ Good — each assertion states what should hold, so failures read as sentences
fn assertions() {
    assert_eq!(actual, expected, "values should be equal");
    assert_ne!(actual, unexpected, "values should be different");
    assert!(condition, "condition should be true");
    assert!(result.is_ok(), "operation should succeed");
    assert!(result.is_err(), "operation should fail");

    // For Option types
    assert!(option.is_some(), "should contain value");
    assert!(option.is_none(), "should be empty");

    // For custom error types
    let error = result.expect_err("operation should fail with invalid input");
    assert!(matches!(error, MyError::ValidationError(_)),
        "Expected ValidationError, got {:?}", error);

    // For Result types
    let value = result.expect("operation should succeed with valid input");
}
```

For collections, assert the shape first, then locate individual items and assert on them.

```rust
// ✅ Good — the length assertion fails first and explains a size mismatch before item lookups panic
#[test]
fn plan_segments_with_three_sections_returns_each_page_aligned() {
    //* Given
    let elf = fixture_elf_with_sections(&[".text", ".rodata", ".data"]);

    //* When
    let segments = plan_segments(&elf).expect("segment planning should succeed");

    //* Then
    assert_eq!(segments.len(), 3, "should plan one segment per section");
    let text = segments.iter()
        .find(|segment| segment.kind == SegmentKind::Text)
        .expect("the text segment should be planned");
    assert_eq!(text.offset % PAGE_SIZE, 0, "the text segment should be page aligned");
    assert!(text.size > 0, "the text segment should not be empty");
}
```

## Checklist

Before submitting a test function for review, verify:

- [ ] Test name follows `<function_name>_<scenario>_<expected_outcome>` format
- [ ] Test name does NOT include the word "test" (it's already marked with `#[test]`)
- [ ] Test uses correct framework: `#[test]` for sync, `#[tokio::test]` for async
- [ ] Test has `//* Given`, `//* When`, and `//* Then` comments (Given optional if no setup needed)
- [ ] Marker comments are keyword-only; explanatory prose goes on a following `//` line
- [ ] `//* When` section calls EXACTLY ONE function under test
- [ ] `//* Then` section contains ONLY assertions and assertion helpers
- [ ] No `unwrap()` calls - all use `.expect("descriptive message")` instead
- [ ] All assertions have descriptive failure messages
- [ ] Test focuses on a single scenario (not testing multiple functions or workflows)
- [ ] Test name is descriptive and explains what is being tested

## References

- [test-files](test-files.md) - Related: Where test modules and files live in the directory structure
- [test-organization](test-organization.md) - Related: Test tier selection (unit, integration, e2e) and how the suites are run
