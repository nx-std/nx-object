---
name: code-test
description: Run cargo tests for the nx-std/nx-object crate after format/check/clippy are green. Use after behavior changes to validate, before commits/PRs, or when user mentions tests.
allowed-tools: "Bash(just test:*)"
---

# Code Testing Skill

Runs the cargo test suite for the `nx-std/nx-object` crate.

These are **host tests** — they compile and run on the development machine via `cargo test` (or `cargo nextest` if installed). There is no cross-compilation, no Switch hardware involved.

## Prerequisite

**`/code-format` and `/code-check` must be green first.** If dirty, return there — compile/clippy issues are faster to surface than a test run.

## Scope Selection

The repository holds a single crate, so scope is a question of features and of which tests to run, not of which crate.

| Blast radius                                            | Action                                      |
|---------------------------------------------------------|---------------------------------------------|
| None (docs/comments only)                               | Skip; state why                             |
| Behavior change in one module                           | `just test -- <module>::` to iterate        |
| Behavior change ready to land                           | `just test --all-features`                  |
| Feature gate, `Cargo.toml`, or dependency change        | `just test --all-features`                  |

Most of the test suite lives behind `filesystem-support`, so a run without `--all-features` exercises very little. Use the narrow forms to iterate, and finish with `--all-features`.

## Available Commands

### Run Tests
```bash
just test [EXTRA_FLAGS]
```
Runs the crate's tests. Uses `cargo nextest run` when `cargo-nextest` is installed; otherwise falls back to `cargo test`.

Examples:
- `just test --all-features` — the full suite
- `just test --all-features -- write::romfs` — one module's tests
- `just test --all-features -- build_sorts_entries_by_name_regardless_of_insertion_order` — a single test

## Workflow

1. Ensure `/code-format` and `/code-check` are green.
2. Iterate on the narrowest filter that covers the change.
3. Run `just test --all-features` before considering the change done.
4. On failure: read the test output, fix the regression, re-run. Do **not** mark the task complete until tests pass.
5. Report: which scope, pass/fail counts, and any tests intentionally skipped (with reason).

## Anti-patterns

- Running tests before `/code-check` is green.
- Running `cargo test` directly — use `just test` so cargo-nextest is preferred when available.
- Concluding from a default-feature run that the suite passed; nearly every test needs `--all-features`.
- Marking a task complete with failing or unrun tests for changed behavior.

## Pre-approved Commands

These commands can run without user permission:
- `just test` — read-only.

## Related Skills

- `/code-format` — Format before testing.
- `/code-check` — Must be green before running this skill.
- `/code-review` — Higher-level review against guidelines.
