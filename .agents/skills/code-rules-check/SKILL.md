---
name: code-rules-check
description: Check a changeset against the code rules in docs/code/. Use after implementing and before committing, or when asked whether code complies with the guidelines.
allowed-tools: Bash(git diff*), Bash(git status*), Bash(git merge-base*), Bash(grep *)
---

# Code Rules Check

Verifies that a changeset follows `docs/code/`. This is a **compliance check, not a review**: it does not hunt
bugs, question the design, or assess security. `/code-review` does that, and runs this check as one of its
dimensions.

One question only: **does this code follow the rules that govern it?**

## 1. The changeset

!`git diff --stat HEAD 2>/dev/null | tail -25`

Uncommitted work is the default subject. For a whole branch use
`git diff $(git merge-base HEAD main)...HEAD`. Check only what the diff touches — an unchanged file that
breaks a rule is not this changeset's finding.

## 2. The rules that govern it

!`grep -m 3 -E '^(description|type|scope):' docs/code/*.md`

Select by what the diff **contains**, not by what the task was about:

| The diff contains | Match triggers from |
|---|---|
| a new or changed signature, constructor, or conversion | `rust-fn*` |
| an error type, a `Result`, a discarded error | `rust-errors-*` |
| `mod`, `use`, or a re-export | `rust-imports`, `rust-mods*` |
| a `///`, `//!`, `//`, or `TODO` | `rust-docs*` |
| `spawn`, `select!`, a lock, or `.await` | `rust-async*` |
| `#[derive]`, `#[expect]`, `#[allow]` | `rust-attrs-*` |
| a manifest, feature gate, or dependency change | `rust-crates` |
| a newtype, builder, or state machine | `pattern-*` |
| parsing external input into a domain type | `rust-parse` |
| a test, fixture, or `#[cfg(test)]` | `test-*` |
| a log line | `logging*` |

Read every match. The `principle-*` descriptions in the catalog already state their rules — design violations
against those are in scope without reading the documents.

## 3. The check

**Every document ends with a `## Checklist`, and those items are the check surface** — the rules restated as
verifiable statements. Walk each item against each changed hunk.

Do this inline when the selection is small. Most changesets are: one coherent piece of work touches a few
rule groups, and the governing documents are usually already in context from `/code-rules` at the start of
the task — reading them again in a subagent would cost more than checking here.

**Escalate to a fan-out when the selection exceeds roughly four rule groups**, or when the diff spans several
crates. Then spawn one agent per group, all in a single message, each given its group's document paths, the
diff command, the instruction to apply those documents' `## Checklist` items, and the report format from §4.
Collect and deduplicate — two groups may flag one line under different rules, which is reported once citing
both.

`/code-review` invokes this skill with the fan-out forced, whatever the diff size, and adds the dimensions no
checklist covers. This skill escalates only when breadth demands it.

## 4. Report

Clean:

> Rules check clean. Applied: `rust-fn`, `rust-errors-reporting`, `rust-docs-rustdoc`.

Violations, most severe first, one per line, with the fix:

> `nx-object/src/read/nro.rs:118` — **rust-errors-reporting**: the error enum is declared before the function
> that returns it. Move `ReadError` after `read_header`.

- **Every finding cites the document that states the rule.** A finding with no document behind it is a style
  opinion — drop it.
- Quote the checklist item when the violation is not self-evident.
- Do not report what the compiler or clippy already catches; `/code-check` owns those.
- Report a rule that seems wrong or contradicts another document as a finding against the *documents*, not
  against the code.

## Not This Skill

| Use | For |
|---|---|
| `/code-review` | bugs, regressions, security, soundness — the deep pass |
| `/code-check` | compile errors and clippy warnings |
| `/code-rules` | loading rules in order to *write* code |
| `/docs-fmt-check` | validating a rule document's own format |
