---
name: code-review
description: Deep review of the working branch — rule compliance, bugs, regressions, security, soundness. Use before opening a PR, or when a change needs scrutiny beyond /code-rules-check.
---

# Code Review

A thorough review of the current branch of the `nx-std/nx-object` crate, run locally. It performs
`/code-rules-check` at review depth — fanned out across rule groups — and adds what a compliance check cannot
see: logic gaps, regressions, security, safety, and soundness.

## When to Use This Skill

- Before opening a PR
- After a large, risky, or long-running implementation
- Reviewing someone else's branch locally
- When `/code-rules-check` is clean but the change still warrants scrutiny

For the routine "does this follow the rules?" pass after finishing a piece of work, use `/code-rules-check`
alone — it is a fraction of the cost and is the gate in the development workflow.

## Review Checklist

Please review this code change and provide feedback on:

### 1. Security & Soundness Concerns

Review for memory-safety and security issues:
- `unsafe` blocks: precondition documentation, invariant maintenance, raw pointer validity, lifetime soundness
- Data races / unsynchronized access in `Sync`/`Send` impls
- Binary parsing in `read` and `raw`: length and offset validation, out-of-bounds reads, integer overflow on
  size and offset fields, trusting attacker-controlled headers
- Zero-copy casts: alignment and size preconditions of every `zerocopy` conversion over caller-supplied bytes
- Path and file handling in `write`: path traversal when a builder walks a directory, symlink following,
  untrusted file names reaching an archive entry
- Input validation at the crate boundary, since every input is a file someone else produced

### 2. Principles Violations

The `principle-*` documents are the design rules, and the `/code-rules` catalog states each one in full — work
from the catalog rather than a list here, which would cover a subset and go stale on the next edit.

Judge the change against all of them. Read a full principle document when you need to *argue* a finding: the
examples and the Pragmatism Caveat are what separate a violation from a deliberate, documented exception, and
a principle finding that ignores the caveat will be rejected.

### 3. Potential Bugs

Look for common programming errors such as:
- Off-by-one errors
- Incorrect conditionals
- Use of wrong variable when multiple variables of same type are in scope
- `min` vs `max`, `first` vs `last`, flipped ordering
- Iterating over hashmap/hashset in order-sensitive operations

### 4. Panic Branches

Identify panic branches that cannot be locally proven to be unreachable:
- `unwrap` or `expect` calls
- Indexing operations
- Panicking operations on external data

**Note**: This overlaps with the error handling rules in `docs/code/rust-errors-handling.md`. Verify
compliance with the project's error handling standards.

### 5. Backwards Compatibility

Verify backwards compatibility is maintained:
- Public API: existing functions, types, and trait signatures must not break consumers
- Cargo `[features]`: removing or renaming a feature breaks consumers, and moving an item behind a gate is the
  same break in a quieter form
- `no_std` support: an item that was reachable without `std` must stay reachable, and nothing ungated may
  start requiring `std` or an allocator
- On-disk format compatibility for the formats the crate reads and writes: a file written by the previous
  version must still parse, and a file this version writes must still load on the console

### 6. Code Rules Compliance

Run `/code-rules-check`, forcing its fan-out path regardless of diff size: one agent per rule group, spawned
in a single message, each applying its documents' `## Checklist` items to the diff. That skill owns the
procedure — which documents govern a change, how groups are derived, and the report format. Do not restate
its rules here; they change when `docs/code/` changes.

A finding in this dimension is a rule violation with a document behind it. Anything a reviewer notices that no
document states belongs in the dimensions above and below, not here.

### 7. Testing

Evaluate test coverage and quality. Tests in this project are standard `cargo test` host tests:
- Reduced test coverage without justification
- Tests that don't actually exercise the new behavior
- Tests with race conditions or non-deterministic behavior
- Changes to existing tests that weaken assertions
- Changes to tests that are actually a symptom of breaking changes to user-visible behaviour
- New public API surface added without a corresponding test

### 8. Performance

Check for performance issues:
- Inefficient algorithms or data structures
- Unnecessary heap allocation in hot paths
- Lock granularity / lock-held-across-IO patterns in async or threaded code
- Synchronous I/O in places that should be async (or vice versa) given the surrounding context

### 9. Documentation

Ensure documentation is up-to-date:
- Public API doc-comments reflect new/changed signatures and `unsafe` preconditions
- `AGENTS.md` reflects any new justfile recipe or workflow change
- README reflects the current feature set and the formats each layer supports
- Crate-level docs reflect added/removed modules

### 10. Dead Code

Find dead code that is not caught by warnings:
- Overriding values that should be read first
- Silently dead code due to `pub`
- `todo!()` or `dbg!()` macros left in production code

### 11. Inconsistencies

Look for inconsistencies between comments and code:
- Documentation that doesn't match implementation
- Misleading variable names or comments
- Outdated comments after refactoring

### 12. Documentation Validation

When the change touches `docs/code/`, invoke `/docs-fmt-check` to validate that each edited rule document
still matches its `docs/__meta__/` specification, and report any format violations in the review.

## Notes

### Focus on Actionable Feedback

- Provide specific, actionable feedback on actual lines of code
- Avoid general comments without code references
- Reference specific file paths and line numbers
- Suggest concrete improvements

### Rule Compliance is Critical

Rule violations should be treated seriously as they:
- Reduce codebase consistency
- Make maintenance harder
- May introduce security vulnerabilities
- Conflict with established architectural decisions

Always run the rule compliance dimension (section 6) as part of every code review.

### Review Priority

Sections are ordered by priority — review from top to bottom:
1. **Security concerns** (§1, highest priority)
2. **Principles violations** (§2)
3. **Potential bugs** and **panic branches** (§3–4)
4. **Backwards compatibility** (§5)
5. **Code rule violations** (§6)
6. **Testing** (§7)
7. **Performance** (§8)
8. **Documentation**, **dead code**, and **inconsistencies** (§9–12)

## Next Steps

After completing the code review:
1. Provide clear, prioritized feedback
2. Distinguish between blocking issues (bugs, soundness, API breaks) and suggestions (style, performance)
3. Reference specific rule documents from `docs/code/` when flagging violations
4. Suggest using `/code-format`, `/code-check`, and `/code-test` skills to validate fixes
