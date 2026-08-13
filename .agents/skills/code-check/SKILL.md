---
name: code-check
description: Validate and lint Rust code after changes in the nx-std/nx-object crate. Use after editing .rs files, when user mentions compilation errors, type checking, linting, clippy warnings, or before commits/PRs. Prefers IDE/rust-analyzer diagnostics when available, defaults to `just` commands, and auto-fixes clippy lints with `--fix`.
allowed-tools: "Bash(just check:*), Bash(just check-rs:*), Bash(just clippy:*), Bash(just check-unused-deps:*), Bash(just check-deps:*), mcp__ide__getDiagnostics, LSP"
---

# Code Checking Skill

Code validation and linting for the `nx-std/nx-object` crate. Optimized for **minimum wall-clock time to first error**: cheapest signal first, auto-fix before hand-fix.

The repository holds a single crate, so there is no per-crate selection to make. What varies here is the **feature set**: the crate is `no_std` with default features and pulls in `std` through `filesystem-support`, so a change can compile in one configuration and fail in the other.

## When to Use This Skill

- Validate Rust code after making changes
- Surface compilation errors quickly
- Lint code with clippy (and auto-apply machine-applicable fixes)
- Ensure code quality before commits or PRs

## Command Selection: Three-Stage Funnel

Run the stages in order. Move to the next stage only after the current one is clean.

```
Stage 0 (optional)                   Stage 1 — mandatory        Stage 2 — mandatory
rust-analyzer-lsp diagnostics   →    just check-rs <flags> →    just clippy <flags> --fix …
(via mcp__ide__getDiagnostics)       once per configuration     once per configuration
```

### The two configurations

The recipes take the flags rather than baking them in, so the same two invocations CI runs are the two you run. These are the flag sets:

| Configuration | Flags                                                       |
|---------------|--------------------------------------------------------------|
| `std`         | `--all-targets --all-features`                               |
| `no-std`      | `--no-default-features --target aarch64-unknown-none`        |

`--all-targets` belongs to the `std` set only: on a bare-metal target the test and bench targets fail to build outright, because they link the libtest harness and want a `#[panic_handler]`.

Run the `std` set always. Add the `no-std` set when any of these is true:

| Signal                     | Meaning                                                                          |
|----------------------------|----------------------------------------------------------------------------------|
| **Feature-gated code edit**| A file under `src/raw/` or `src/read/`, or any code not behind `filesystem-support`. |
| **New import**             | Any `use` added outside a `filesystem-support` gate. `std` is unavailable there.  |
| **Feature or gate change** | `[features]` in `Cargo.toml`, or a `#[cfg(feature = ...)]` added, moved, or removed. |
| **New dependency**         | A dependency added, or an existing one's `default-features`/`features` changed.   |

The `std` set compiles for the host, where `std` exists. The `no-std` set cross-compiles to `aarch64-unknown-none`, which has no `std` at all, so a dependency that quietly enables `std` (a feature that unified the wrong way, a `default-features` flag left on) fails there and only there.

The target stands in for the console's own `aarch64-nintendo-switch-freestanding`, which is tier 3 and would need `-Z build-std` on nightly. Install it once with `rustup target add aarch64-unknown-none`.

## Stage 0 — Rust-analyzer Diagnostics (Fast Path)

Stage 0 reads diagnostics rust-analyzer has already computed in the background. It is near-instant because it does not invoke cargo.

**Plugin:** The Claude Code plugin that provides Rust language-server capabilities is **`rust-analyzer-lsp`** (from the `claude-plugins-official` marketplace). When installed it powers the `LSP` tool for `.rs` files and feeds rust-analyzer diagnostics into the conversation via `mcp__ide__getDiagnostics`.

**Probe availability.** Call `mcp__ide__getDiagnostics` once with no `uri`. If it returns (even an empty array), diagnostics are live — proceed. If it errors or is not available, skip Stage 0 and go to Stage 1.

**Per-file diagnostics.** For each edited Rust file, call `mcp__ide__getDiagnostics` with `uri=file://<absolute-path>`. Fix reported errors and warnings before moving on.

**Stage 0 is advisory, never terminal.** Stage 1 remains mandatory even when Stage 0 is clean: rust-analyzer may be stale, and it reports one feature configuration only.

## Available Commands

### Check Rust Code
```bash
just check-rs [EXTRA_FLAGS]
```
Runs `cargo check` with whatever flags you pass. **Default Stage 1 command.** **Alias:** `just check`.

Examples:
- `just check-rs --all-targets --all-features` — the `std` configuration
- `just check-rs --no-default-features --target aarch64-unknown-none` — the `no-std` configuration

### Lint Rust Code with Auto-fix
```bash
just clippy [EXTRA_FLAGS]
```
Runs `cargo clippy` with whatever flags you pass. **Default Stage 2 command.**

Examples:
- `just clippy --all-targets --all-features --fix --allow-dirty --allow-staged` — standard auto-fix pass
- `just clippy --all-targets --all-features` — residue pass (after `--fix`) to surface remaining warnings
- `just clippy --no-default-features --target aarch64-unknown-none -- -D warnings` — the `no-std` lint CI runs

#### Auto-fix semantics

`cargo clippy --fix` automatically rewrites the source to apply all *machine-applicable* suggestions (unused imports, redundant clones, idiomatic rewrites, etc.). After the auto-fix pass:
- **Residual warnings remain for hand-fixing.** Re-run the same command **without `--fix`** to list the residue, then hand-fix.
- **Formatting may shift.** Re-run `/code-format` after `--fix` applies changes.
- **`--allow-dirty --allow-staged` are required** because the dev workflow always has uncommitted changes when this skill runs; without them cargo refuses to modify files.

## Important Guidelines

### MANDATORY: Run Checks After Changes

You MUST run checks after making code changes. Follow the three-stage funnel above.

Before considering a task complete: all checks MUST pass AND all clippy warnings MUST be fixed (either auto-fixed or hand-fixed).

### Example Workflows

**Common case (write-layer edit):**
Edits in `src/write/...`, which is behind `filesystem-support`.

1. Format changes: use `/code-format`.
2. **Stage 0** — probe `mcp__ide__getDiagnostics`. If available, call with each edited file's `file://` URI. Fix reported issues.
3. **Stage 1** — `just check-rs --all-targets --all-features` → fix errors → repeat until clean.
4. **Stage 2** — `just clippy --all-targets --all-features --fix --allow-dirty --allow-staged`
   - If warnings remain: re-run without `--fix`, hand-fix the residue.
5. Re-run `/code-format` if `--fix` changed source.
6. Done when: zero errors AND zero warnings.

**`no_std` case (raw or read layer edit):**
Edits in `src/raw/...`, which compiles in every configuration.

1. Format changes: use `/code-format`.
2. **Stage 0** — probe `mcp__ide__getDiagnostics`; fix surfaced issues.
3. **Stage 1** — run both configurations → fix errors → repeat until both are clean:
   - `just check-rs --all-targets --all-features`
   - `just check-rs --no-default-features --target aarch64-unknown-none`
4. **Stage 2** — `just clippy --all-targets --all-features --fix --allow-dirty --allow-staged`, hand-fix the
   residue, then lint the other configuration:
   `just clippy --no-default-features --target aarch64-unknown-none`
5. Re-run `/code-format` if `--fix` changed source.
6. Done when: zero errors AND zero warnings in both configurations.

## Common Mistakes to Avoid

### Anti-patterns
- **Never run `cargo check` or `cargo clippy` directly** — go through `just check-rs` / `just clippy`, so the command a developer runs and the command CI runs stay the same one.
- **Never pass `--all-targets` to the `no-std` configuration** — the test and bench targets cannot build on a bare-metal target.
- **Never assume the `std` check covers `no_std`** — it compiles for the host, where `std` is available to every dependency.
- **Never skip Stage 1 just because Stage 0 is clean** — rust-analyzer may be stale.
- **Never run clippy without `--fix` on the first pass** — wastes cycles on machine-applicable lints.
- **Never pass `--fix` without `--allow-dirty --allow-staged`** — cargo refuses to modify files in a dirty tree.
- **Never ignore clippy warnings** — fix-all-warnings is mandatory.

### Best practices
- Start with Stage 0 when `rust-analyzer-lsp` is installed and `mcp__ide__getDiagnostics` responds.
- Check with `--all-features` so the gated paths compile at all; without it the `write` layer is not built.
- Always use `--fix --allow-dirty --allow-staged` on the first clippy pass; re-run without `--fix` to list residue.
- Fix compilation errors (Stage 1) before running clippy (Stage 2).
- Run the full funnel when you finish a coherent chunk of work or before committing.

## Pre-approved Commands

These commands can run without user permission:
- `mcp__ide__getDiagnostics` — read-only.
- `LSP` tool operations against `.rs` files — read-only.
- `just check-rs` (alias `just check`) in either configuration — safe, read-only.
- `just check-unused-deps` (alias `just check-deps`) — runs `cargo machete`; read-only.
- `just clippy` in either configuration — safe, read-only.
- `just clippy --fix --allow-dirty --allow-staged` — auto-apply of machine-applicable fixes; affects only source files already being edited.

## Related Skills

- `/code-format` — Format code before/after running checks.
- `/code-test` — Run tests after checks are green.
- `/code-review` — Higher-level review against guidelines.
