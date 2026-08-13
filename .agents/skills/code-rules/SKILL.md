---
name: code-rules
description: Load the code rules that apply to the work at hand, from docs/code/. Use before planning or writing code, or when asked about conventions, standards, or design principles.
allowed-tools: Bash(grep *)
---

# Code Rules

`docs/code/` holds 42 rule documents, ~75k tokens in total. This skill loads only their frontmatter, and you
choose what to read from it. Selecting well is the whole job: read what the task needs, nothing more.

## Catalog

!`grep -m 3 -E '^(description|type|scope):' docs/code/*.md`

> If the block above is literal text, the runtime did not execute it — run that grep yourself first.

## Selecting

Every `description` ends with `Load when …`, naming the situations that document governs. Match the task
against those clauses, then:

- **Take the most specific match.** A prefix is a group (`rust-docs-*`, `rust-fn-*`, `rust-errors-*`,
  `rust-async-*`, `rust-mods-*`, `test-*`). Read the member whose trigger fits — `rust-docs-rustdoc` for a
  `///` block, not the group. Add the parent only when the task turns on what the parent owns: `rust-docs`
  decides which channel a fact belongs in, `rust-mods` decides which file an item belongs in.
- **Expect two to four documents.** One is common. More than four means the task is unscoped, or you are
  matching topics instead of triggers.
- **Break ties by specificity:** `type: core`, then `arch`, then `principle`. Read `type: meta` only when
  writing a rule document itself.
- **Read nothing adjacent.** If no trigger matches, say so — a gap in the corpus is worth reporting.

Read selections at `docs/code/<name>.md`. Do not re-read what is already in context.

## Principles Are Transversal

The `principle-*` rules apply to all design work, so they cannot be trigger-matched like the rest — nobody
describes their task as "a coupling concern". They do not need to be: **every `principle-*` description above
states its rule in full**, so the catalog you already have _is_ the principles digest. Design against every
one of them without reading any of them.

Read a full principle document only to **argue** one — to justify a decision, settle a disagreement, or cite
it in review. That is what the examples, the Why It Matters, and the Pragmatism Caveat are for; the rule
itself is already in hand. On `/code-rules principles`, read them all and summarise.

## Not This Skill

| Use | For |
|---|---|
| `/code-rules-check` | auditing a finished changeset against the rules |
| `/code-format`, `/code-check`, `/code-test` | running formatters, lints, tests |
| `/code-review` | bugs, regressions, security, soundness — the deep pass |
| `/docs-fmt-check` | validating a rule document's own format |
