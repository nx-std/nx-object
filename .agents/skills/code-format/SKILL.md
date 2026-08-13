---
name: code-format
description: Format Rust code in the nx-std/nx-object crate. Use immediately after editing .rs files, when user mentions formatting, code style, rustfmt, or before commits/PRs.
allowed-tools: "Bash(just fmt:*), Bash(just fmt-check:*)"
---

# Code Formatting Skill

Code formatting operations for the `nx-std/nx-object` crate.

## When to Use This Skill

- Format code after editing Rust files
- Check if code meets formatting standards
- Ensure code formatting compliance before commits

## Command Selection Rules

This is a single small crate. Always format the whole crate — it is fast.

| Scope    | Command    | Rationale                              |
|----------|------------|----------------------------------------|
| Any edit | `just fmt` | Crate-wide nightly rustfmt; standard.  |

## Available Commands

### Format Rust Code
```bash
just fmt
```
Formats all Rust code using `cargo +nightly fmt --all`. Nightly is required because `rustfmt.toml` uses unstable features (`imports_granularity`, `group_imports`).

### Check Rust Formatting
```bash
just fmt-check
```
Checks formatting without making changes (`cargo +nightly fmt --all -- --check`).

## Important Guidelines

### Format Before Checks/Commit

Format when you finish a coherent chunk of work and before running checks or committing.

### Example Workflow

1. Edit `src/...`.
2. Run `just fmt`.
3. Run `/code-check`.

## Common Mistakes to Avoid

### Anti-patterns
- **Never run `cargo fmt` or `rustfmt` directly** — use `just fmt` (selects nightly + project config).
- **Never skip formatting before checks/commit** — even minor edits.
- **Never commit unformatted code** — verify with `just fmt-check`.

### Best Practices
- Format before running checks/tests or before committing.
- Run `just fmt-check` to verify formatting before commits.

## Formatting Configuration

Nightly rustfmt (pinned via `rust-toolchain.toml`), config in `rustfmt.toml` with unstable features (import grouping std/external/local, crate-level granularity).

## Pre-approved Commands

These commands can run without user permission:
- `just fmt` — safe formatting operation.
- `just fmt-check` — safe, read-only format check.

## Next Steps

After formatting:
1. **Check compilation** → `/code-check`
2. **Run clippy** → `/code-check`
3. **Run tests when warranted** → `/code-test`
