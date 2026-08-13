---
name: "rust-mods-members"
description: "Member ordering within a module: main API first, errors after the function that returns them. Load when organizing module contents"
type: core
scope: "global"
---

# Module Member Ordering

**MANDATORY for ALL Rust code in this workspace**

A module reads top-down as its public interface first, then the code that supports it, so a reader meets the
API before the machinery and a reviewer finds the main logic without scrolling.

## Member Ordering

Organize module members in this order:

1. **Module prologue** — doc comment, imports, `mod` declarations, re-exports ([rust-imports](rust-imports.md))
2. **Constants and statics** (`const`, `static`)
3. **Type aliases** (`type Foo = ...`)
4. **Main module members** — public types, main functions (`run`, `execute`, `new`)
5. **Helper types and functions** — in dependency order: if A calls B, A comes first

The prologue itself — the `//!` block, the import groups, the `mod` declarations, and the local re-exports —
is owned by [rust-imports](rust-imports.md). This document covers the items that follow it.

```rust
// ❌ Bad — the reader meets a private result struct and a helper before learning what the
// module is for, and has to read the whole file to find the one function callers use.
struct HelperResult { ... }
fn helper_function() { ... }
pub async fn run() { ... }
pub enum Error { ... }
```

```rust
// ✅ Good — the entry point first, then its failures, then the machinery it calls.
pub async fn run() { ... }
pub enum Error { ... }

struct HelperResult { ... }
fn helper_function() { ... }
```

## Error Types Follow Their Function

An error type is declared **immediately after** the function or `impl` block that returns it, never before it
and never in a separate module. The function is what a reader came for; its error is the detail they need
second, and keeping the pair adjacent means a change to a failure path is a single-file edit.

```rust
// ❌ Bad — the error precedes the function, so the reader meets six variants before
// learning what produces them.
#[derive(Debug, thiserror::Error)]
pub enum SendNroError { /* ... */ }

pub async fn send_nro(dst: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {}
```

```rust
// ✅ Good — the function first, then the failures it can produce.
pub async fn send_nro(dst: SocketAddr, nro: &[u8]) -> Result<(), SendNroError> {}

/// Errors returned by [`send_nro`].
#[derive(Debug, thiserror::Error)]
pub enum SendNroError { /* ... */ }
```

Which module the error lives in, and why an `error.rs` collection is not one, is owned by
[rust-errors-reporting](rust-errors-reporting.md).

## Common Violations

- Main public function (`run`, `main`, `execute`) buried after helper functions
- An error type separated from the function that returns it, or collected at the end of the file
- Helper structs or functions appearing before the main types they support
- Private implementation details scattered before public API

## Checklist

Before committing Rust code, verify:

- [ ] Main public function (`run`, `execute`, etc.) appears early in the file
- [ ] Public structs/types appear before private helpers
- [ ] Each error type sits immediately after the function or `impl` that returns it
- [ ] Helper functions appear after the code that uses them
- [ ] No private implementation details scattered before public API

## References

- [rust-imports](rust-imports.md) - Related: The module prologue that precedes these members
- [rust-mods](rust-mods.md) - Extends: The module invariants these rules make operational
- [rust-mods-files](rust-mods-files.md) - Related: Module file layout and the no-`mod.rs` rule
- [rust-mods-graph](rust-mods-graph.md) - Related: Which references between module files are legal
