---
name: "rust-errors-reporting"
description: "Declaring error types with thiserror: variant forms, #[source], one enum per function, declared beside it. Load when defining an error type or variant"
type: core
scope: "global"
---

# Error Reporting Patterns

**MANDATORY for ALL error handling in this workspace**

## 1. Derive `thiserror::Error` Fully Qualified

**ALWAYS** write `#[derive(Debug, thiserror::Error)]`.

```rust
// ✅ Good — resolves to the derive macro no matter what `Error` means in this module
#[derive(Debug, thiserror::Error)]
pub enum BundleNspError { /* ... */ }

// ❌ Bad — the import collides with any local `Error` type and the derive silently
// resolves to whichever won the name
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error { /* ... */ }
```

## 2. Enum for Several Sources, Struct for One

An enum when the operation has multiple distinct failure modes; a struct when it has exactly one.

```rust
// ✅ Good — enum: the operation fails in two distinguishable ways
#[derive(Debug, thiserror::Error)]
pub enum BundleNspError {
    #[error("Failed to build the program descriptor")]
    NpdmBuild(#[source] NpdmBuildError),

    #[error("Failed to write the bundle to the output path")]
    WriteOutput(#[source] std::io::Error),
}

// ✅ Good — struct: a single source needs no variant to select between
#[derive(Debug, thiserror::Error)]
#[error("Failed to read the linked ELF")]
pub struct ReadElfError(#[source] pub std::io::Error);
```

## 3. Variant Forms: Tuple by Default, Named for Context

**ALWAYS** use tuple form for a single-field variant. Use named fields only when the message carries context
alongside the source.

```rust
// ✅ Good — tuple form; a lone source field gains nothing from a name
#[derive(Debug, thiserror::Error)]
pub enum ReadNroError {
    #[error("Failed to read the NRO from disk")]
    Read(#[source] std::io::Error),

    #[error("Failed to parse the NRO image")]
    Parse(#[source] nx_object::read::NroFromBytesError),
}

// ✅ Good — named fields; the message needs values the source does not carry
#[derive(Debug, thiserror::Error)]
pub enum PackAssetsError {
    #[error("Invalid RomFS entry path '{path}'")]
    InvalidEntryPath { path: String, source: EntryPathError },

    #[error("Failed to read asset '{path}' for target '{target}'")]
    ReadAsset { target: String, path: String, source: std::io::Error },
}
```

## 4. Wrap Source Errors in Domain Variants

**ALWAYS** wrap an underlying error in a variant that names what this layer was attempting. Returning a
dependency's error type propagates the failure without the step that caused it.

```rust
// ✅ Good — every failure arrives as a variant naming the step that produced it
pub fn bundle_nsp(&self, spec: &NpdmSpec) -> Result<Vec<u8>, BundleNspError> {
    let npdm = NpdmBuilder::from(spec)
        .build()
        .map_err(BundleNspError::NpdmBuild)?;

    let romfs = RomFsBuilder::new()
        .add_dir(&self.romfs_dir)
        .map_err(BundleNspError::RomFsScan)?;

    // ...
}

// ❌ Bad — the caller receives a bare io error and cannot tell whether the descriptor
// failed to build or the asset directory failed to scan
pub fn bundle_nsp(&self, spec: &NpdmSpec) -> Result<Vec<u8>, std::io::Error> {
    let npdm = NpdmBuilder::from(spec).build()?;
    // ...
}
```

## 5. No `#[from]`, No `From` Implementations

**DO NOT** use `#[from]` or write a manual `From` impl unless explicitly required. Explicit `.map_err()` shows
where wrapping happens and prevents an unrelated call from silently converting into a variant that misnames it.

```rust
// ✅ Good — the wrapping site is visible at the call
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Failed to send the file name to the console")]
    SendFileName(#[source] std::io::Error),
}

send_file_name(&mut sock, name)
    .await
    .map_err(SendNroError::SendFileName)?;

// ❌ Bad — every `?` on an `io::Error` in the function becomes this variant, so
// a failure writing the payload reports itself as a failed file name
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Failed to send the file name to the console")]
    SendFileName(#[from] std::io::Error),
}

send_file_name(&mut sock, name).await?;
```

## 6. Always Mark the Source With `#[source]`

**MANDATORY**: every wrapped error is reachable through `std::error::Error::source()`. A field that is not
annotated ends the chain, so the cause never reaches a log line or an error response.

```rust
// ✅ Good — the chain survives to the formatter
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Failed to connect to the console")]
    Connect(#[source] std::io::Error),
}

// ❌ Bad — the cause is stored but invisible: `.source()` returns `None` and the log
// says only "Failed to connect to the console"
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Failed to connect to the console")]
    Connect(std::io::Error),
}
```

With named fields, `thiserror` treats a field named `source` as the source automatically, so the attribute is
redundant (but harmless) there. **Prefer naming the field `source`.** Any other name (`error`, `inner`,
`cause`) **MUST** carry `#[source]`.

```rust
// ✅ Good — field named `source`; the attribute may be written or omitted
#[derive(Debug, thiserror::Error)]
pub enum WriteBundleError {
    #[error("Failed to write the bundle to '{path}'")]
    Write { path: String, source: std::io::Error },
}

// ❌ Bad — field not named `source` and not annotated, so the chain ends here
#[derive(Debug, thiserror::Error)]
pub enum WriteBundleError {
    #[error("Failed to write the bundle to '{path}'")]
    Write { path: String, error: std::io::Error },
}
```

## 7. Never Embed the Source in the Display String

**MANDATORY**: when a field is the `#[source]`, do **NOT** reference it from `#[error("...")]` via `{0}`,
`{1}`, or `{source}`. Chain formatters (`error_with_causes`, `logging::error_source`, `ErrorResponse::from`)
already append `.source()`, so embedding it prints the same sentence twice in every log line and response body.
Context fields other than the source are included as normal.

```rust
// ✅ Good — the message describes this level only; the cause arrives via the chain
#[derive(Debug, thiserror::Error)]
#[error("Invalid address for {name}")]
pub struct InvalidAddrError {
    pub name: String,
    #[source]
    pub source: std::net::AddrParseError,
}

// ❌ Bad — renders as "Invalid address for api: invalid socket address | Caused by:
// invalid socket address"
#[derive(Debug, thiserror::Error)]
#[error("Invalid address for {name}: {source}")]
pub struct InvalidAddrError {
    pub name: String,
    #[source]
    pub source: std::net::AddrParseError,
}
```

## 8. Name the Closure Parameter `err`

**ALWAYS** bind the error as `err` in `.map_err()`, **NEVER** `e`, unless it shadows a binding already in scope.

```rust
// ✅ Good — the closure names the value it binds
std::fs::read(&asset_path)
    .map_err(|err| PackAssetsError::ReadAsset {
        target: target.clone(),
        path: asset_path.display().to_string(),
        source: err,
    })?;

// ✅ Good — a tuple variant needs no closure at all
std::fs::read(&nro_path).map_err(ReadNroError::Read)?;

// ❌ Bad — a single letter that says nothing about what it holds, in a closure long
// enough that the reader has to scroll back to find out
std::fs::read(&asset_path)
    .map_err(|e| PackAssetsError::ReadAsset {
        target: target.clone(),
        path: asset_path.display().to_string(),
        source: e,
    })?;
```

## 9. One Variant Per Error Source

**NEVER** reuse one variant for more than one error source. Each variant describes a single, specific failure
condition.

```rust
// ✅ Good — the variant name identifies which step failed
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Failed to connect to the console")]
    Connect(#[source] std::io::Error),

    #[error("Failed to send the file name and length")]
    SendFileName(#[source] std::io::Error),

    #[error("Failed to send the compressed file data")]
    SendFileData(#[source] std::io::Error),
}

// ❌ Bad — every socket call reports the same variant, so a user reading
// "Transfer error" cannot tell a refused connection from a truncated payload
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    #[error("Transfer error")]
    Transfer(#[source] std::io::Error),
}
```

## 10. One Error Enum Per Fallible Function

**Prefer** one error type per fallible function or closely related operation. Reuse a type only when the
sharing functions can return **ALL** of its variants.

```rust
// ✅ Good — dedicated error type per operation
pub fn bundle_nsp(&self, spec: &NpdmSpec) -> Result<Vec<u8>, BundleNspError> { /* ... */ }
pub async fn send_nro(&self, nro: &[u8]) -> Result<(), SendNroError> { /* ... */ }

// 🔶 Acceptable — shared type where both functions can return both variants
#[derive(Debug, thiserror::Error)]
pub enum SegmentError {
    #[error("Segment {index} extends past the end of the image")]
    OutOfBounds { index: usize },

    #[error("Segment {index} has an offset and size that overflow")]
    BoundsOverflow { index: usize },
}

// ❌ Bad — variants are half-unreachable from each caller, so every `match` handles
// cases the function it called cannot produce
#[derive(Debug, thiserror::Error)]
pub enum SharedError {
    #[error("Failed to parse the NPDM descriptor")]
    NpdmParse(#[source] NpdmParseError), // only `bundle_nsp` returns this

    #[error("No console found on the network")]
    NoConsoleFound, // only `send_nro` returns this
}
```

## 11. Errors Live With the Function That Returns Them

**MANDATORY**: an error type is declared in the same module as the function that returns it, immediately
**after** that function or `impl` block.

The error and the function are one unit: the variants enumerate exactly the ways that function fails, so a
change to either is a change to both. Splitting them across files means every edit to a failure path is a
two-file edit, and the compiler cannot tell you when they drift apart.

```rust
// ✅ Good — the error follows the function it belongs to
pub fn bundle_nsp(&self, spec: &NpdmSpec) -> Result<Vec<u8>, BundleNspError> {
    // ...
}

/// Errors returned by [`bundle_nsp`].
#[derive(Debug, thiserror::Error)]
pub enum BundleNspError {
    #[error("Failed to build the program descriptor")]
    NpdmBuild(#[source] NpdmBuildError),

    #[error("Failed to write the bundle to the output path")]
    WriteOutput(#[source] std::io::Error),
}
```

**An `error.rs` module is not a home for a collection of error types.** It is permitted only for an error the
module itself owns — a crate-level `Error` that its public API returns, or a shared response type — and that
file holds **one** type.

```rust
// ❌ Bad — error.rs as a bucket. Adding a failure path becomes a two-file edit, and a
// variant that stops being constructed is invisible because nothing nearby shows what
// still returns it.
//
// src/error.rs
pub enum BundleNspError { /* ... */ }
pub enum ReadNroError { /* ... */ }
pub enum PackAssetsError { /* ... */ }

// 🔶 Acceptable — error.rs holding the one error the crate itself surfaces
//
// src/error.rs
/// Errors surfaced by this crate's public API.
#[derive(Debug, thiserror::Error)]
pub enum Error { /* ... */ }
```

The test is ownership, not the filename: ask which function's failure this type describes. If the answer names
one function, the type belongs next to it. If the answer is "the crate", `error.rs` is where it lives.

## 12. No Unused Error Variants

**MANDATORY**: every variant is constructed somewhere. Remove one that is not, immediately.

```rust
// ❌ Bad — `NotFound` is never constructed, so every caller writes a match arm for a
// failure that cannot happen, and the type lies about what the function does
#[derive(Debug, thiserror::Error)]
pub enum ReadNroError {
    #[error("Failed to read the NRO from disk")]
    Read(#[source] std::io::Error),

    #[error("NRO not found")]
    NotFound,
}
```

## 13. Error Documentation Template

**MANDATORY**: document each variant as brief description, when it occurs, optional causes, optional
guarantees (transaction semantics, retry safety).

```rust
#[derive(Debug, thiserror::Error)]
pub enum SendNroError {
    /// The console rejected the transfer before any data was sent
    ///
    /// Occurs when the console has less free space than the declared file length.
    /// Detected via the acknowledgement code the console returns for the file name
    /// and length exchange, before the first chunk is written.
    #[error("The console has insufficient space for '{0}'")]
    InsufficientSpace(String),

    /// Failed to send the compressed file data after the transfer was accepted
    ///
    /// The console discards a partially received file, so nothing is left behind
    /// and no half-written NRO can be launched.
    ///
    /// Possible causes:
    /// - The console left the network mid-transfer
    /// - The console's receive buffer filled and the socket timed out
    /// - The local file shrank while it was being read
    ///
    /// Safe to retry from the beginning: no partial state was persisted.
    #[error("Failed to send the compressed file data")]
    SendFileData(#[source] std::io::Error),
}
```

## 14. No `BoxError` or `Box<dyn Error>`

**DO NOT** use `BoxError`, `Box<dyn Error>`, or similar type erasure in production code. It discards the type,
so callers cannot match on the failure and the structure of the error is invisible at the signature.

```rust
// ✅ Good — the source type is part of the contract, so a caller can match on it
#[derive(Debug, thiserror::Error)]
pub enum PackAssetsError {
    #[error("Failed to read asset '{path}' for target '{target}'")]
    ReadAsset { target: String, path: String, source: std::io::Error },
}

// ❌ Bad — a caller that must distinguish "file missing" from "permission denied" is
// left with a string comparison on the message
#[derive(Debug, thiserror::Error)]
pub enum PackAssetsError {
    #[error("Failed to read asset")]
    ReadAsset { target: String, path: String, source: BoxError },
}
```

**Exception:** prototyping and proof-of-concept work may use `BoxError` temporarily. It **MUST** be replaced
with concrete types before merging to main, carry a `TODO` naming the replacement, and be raised in review.

```rust
// 🔶 Acceptable — prototype only, and the TODO is what keeps it from shipping
#[derive(Debug, thiserror::Error)]
pub enum PrototypeError {
    // TODO: replace BoxError with a concrete source type before production
    #[error("Failed to decompress the NSO segment")]
    Decompress(BoxError),
}
```

## Checklist

Before committing error handling code, verify:

- [ ] All error types use `#[derive(Debug, thiserror::Error)]`
- [ ] Enums used for multiple error sources, structs for single sources
- [ ] Tuple form used for single-field variants (unless named fields provide context)
- [ ] All underlying errors are wrapped with domain-specific variants
- [ ] No `#[from]` attributes or `From` implementations (unless explicitly required)
- [ ] All wrapped errors use `#[source]`, or are named `source` in a named-field variant
- [ ] Source fields are NOT referenced in `#[error("...")]` format strings (no `{0}`, `{source}` when `#[source]` is present)
- [ ] Closure parameters in `.map_err()` are named `err` (not `e`)
- [ ] Each error variant is used for a single, distinct error source
- [ ] One error type per function (or shared only when all variants are common)
- [ ] Each error type is declared in the same module as the function that returns it, immediately after it
- [ ] No `error.rs` holds a collection of per-function error types; it holds at most the one error the module
      itself owns
- [ ] No unused error variants exist
- [ ] All error variants are fully documented following the template
- [ ] No `BoxError` or `Box<dyn Error>` in production code

## References

- [rust-errors-handling](rust-errors-handling.md) - Related: Propagating and recovering from the errors declared here
