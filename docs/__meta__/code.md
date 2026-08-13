---
name: "code"
description: "Code rules documentation format specification. Load when creating or editing rule documents in docs/code/"
type: "meta"
scope: "global"
---

# Code Rules Documentation Format

**MANDATORY for ALL rule documents in `docs/code/`**

## Table of Contents

1. [Core Principles](#1-core-principles)
2. [Frontmatter Requirements](#2-frontmatter-requirements)
3. [Naming Schema](#3-naming-schema)
4. [Cross-Reference Rules](#4-cross-reference-rules)
5. [Document Structure](#5-document-structure)
6. [Content Guidelines](#6-content-guidelines)
7. [Template](#7-template)
8. [Checklist](#8-checklist)

---

## 1. Core Principles

The corpus in `docs/code/` is **the code rules**, also called the code guidelines; the two names mean the same thing, and a single member of it is a **rule document**.

### Rule Documents Are Authoritative

**CRITICAL**: The code rules are the **ground truth** for how code should be written.

- If a rule document exists, the implementation **MUST** follow it
- If code diverges from a documented rule, the code is wrong OR the rule must be updated
- Engineers **MUST** keep rule documents accurate - outdated rules are unacceptable
- When rules evolve, update the rule document in the same change

### Rules Describe This Codebase

Rule documents describe conventions that **exist in this workspace's crates**, not conventions imported from other ecosystems or aspirational ones.

- Before writing a rule, find the code that already demonstrates it — then write the example from scratch, without citing that code ([§6](#6-content-guidelines))
- Do not document tooling the repository does not use
- If a rule proposes a new convention, apply it to the code in the same change

The demonstrator is a check the **author** performs, not a citation the **doc** carries. A convention nothing demonstrates is not a convention this repository has; a doc that proves it by quoting a module has merely made a copy that will drift.

### One Document, One Responsibility

**A rule document has exactly one reason to change.** The Single Responsibility Principle applies to these documents as it does to the modules they govern.

Decide a rule's home by asking what would force it to be rewritten:

| The rule changes when…          | It belongs in…                   |
|---------------------------------|----------------------------------|
| Declaration syntax changes      | The group's declaration document |
| A call-time API changes         | The group's usage document       |
| A naming convention changes     | `rust-fn`                        |
| The build or manifest changes   | `rust-crates`                    |
| A third-party signature changes | The document that owns that seam |

**Each rule has exactly one home.** Siblings **link**; they do not restate. A rule stated in two docs has two places to rot and no authority when they disagree — the reader cannot tell which one is stale. Cross-reference with a one-line pointer instead of repeating the rule or its example.

A group's parent doc (`rust-mods`, `logging`) is itself a **rule document with content**, not an index. Do not add a doc whose only job is to route to its siblings: frontmatter discovery already does that, and a hand-maintained routing table is a second source of truth that goes stale silently.

### A Group's Prefix Names Its Subject

A rule filed under a prefix must be **about that subject**. `rust-errors-*` is about how this workspace declares and surfaces errors; a rule that merely _returns_ a `Result` does not belong there. Ask what the rule is about, not what it touches — a doc about admin API handlers is about the admin API, however much `thiserror` appears in it.

### Rules Are Conventions, Not Module Facts

`docs/code/` carries conventions that **generalize across the workspace**. A rule that applies to exactly one module or one third-party seam is not a convention: it is a fact about that code, and it belongs **in that code** — a module-level `//!` doc block, or an inline comment at the declaration, both governed by the `rust-docs` group.

The test is where a reader needs it. Someone editing the NRO header writer is looking at that writer's module, not searching `docs/code/` — so the alignment invariant the container format forces on it, and the comment that must accompany it, are documented at the declaration. A rule that restates a single module's contract is a copy that drifts from the code it describes.

Promote a module fact to a rule only when it recurs across crates and a reader must apply it to code they have not seen yet.

### Rule Documents State Rules, Not Records

**A rule document states the rule in the imperative present.** It is not a record of how the codebase got here.

Write "untrusted input is validated at the boundary", not "we are adopting X" or "this replaces the hand-written guards we used to carry". Migration narrative, the case for a past decision, and rebuttals of the alternatives are **commit messages and PR descriptions**, not rules. A reader arriving in a year needs to know what to type, not what was argued.

Rules must be **independent of workspace status** — anything true only of today's snapshot rots silently, because nobody re-checks a doc. Keep these **out** of `docs/code/`:

| Do not document                                     | Because                                          | Put it in                     |
|-----------------------------------------------------|--------------------------------------------------|-------------------------------|
| Dependency versions, pins, beta/release status      | Changes on every upgrade                         | Commit message or PR description |
| Benchmark figures and measured timings              | Measured once, on one machine, never re-measured | Commit message or PR description |
| Whether a tool is installed, or its install command | Setup state, not a coding rule                   | Commit message or PR description |
| An inventory of every site a rule applies to        | Must be edited whenever a site is added          | Cite one example              |

Citing a file that **demonstrates** a convention is required (above) and remains so. **Enumerating every instance** of it is an inventory, and an inventory is a maintenance burden that a rule document does not need: cite the clearest example and state the test the reader applies to their own case.

Linking to an external source (a paper, a spec, a canonical blog post) is fine — see the `External References` section in the `principle-*` docs. Linking to a dependency's release notes, migration map, or install instructions is status.

### Discoverability Through Frontmatter

Rule documents use YAML frontmatter for lazy loading - AI agents query frontmatter to determine which rules to load based on the current task context.

### Consistency and Machine Readability

This format specification ensures:

- **Uniform structure** across all rule documents
- **Machine-readable metadata** for automated discovery
- **Clear categorization** via rule types and scopes for organized access
- **Scalability** - easy to add new rules following established format

### Avoid Context Bloat

Keep rule documents focused and concise. Agent entrypoint docs should NOT hardcode rule lists - use dynamic discovery instead.

---

## 2. Frontmatter Requirements

**CRITICAL**: Every rule document MUST begin with valid YAML frontmatter:

```yaml
---
name: "rule-name-kebab-case"
description: "Brief description. Load when [trigger conditions]"
type: "principle|core|arch|crate|meta"
scope: "global|crate:<name>"
---
```

### Field Requirements

| Field         | Required | Format                       | Description                                                            |
|---------------|----------|------------------------------|------------------------------------------------------------------------|
| `name`        | YES      | kebab-case                   | Unique identifier matching filename (minus .md)                        |
| `description` | YES      | Single line, succinct        | Discovery-optimized description (see Description Guidelines below)     |
| `type`        | YES      | `principle`, `core`, `arch`, `crate`, or `meta` | Rule category (see Type Definitions below)               |
| `scope`       | YES      | `global` or `crate:<name>`   | Application scope: global or crate-specific                            |

**All four values are double-quoted**, as the block above writes them. YAML accepts a bare `type: core`, so
the two forms coexist happily and drift apart silently; one form means a diff on the field is always a change
of value, never a change of style.

### Type Definitions

| Type   | Purpose                          | Scope           | Characteristics                                      |
|--------|----------------------------------|-----------------|------------------------------------------------------|
| `principle` | Universal software principles | Always `global` | Best practices for optimal code quality              |
| `core` | Fundamental coding patterns      | Always `global` | Applicable across entire codebase                    |
| `arch` | Architectural patterns           | Always `global` | High-level organizational and structural patterns    |
| `crate`| Crate-specific patterns          | `crate:<name>`  | Patterns for individual crates or modules            |
| `meta` | Documentation about documentation| Always `global` | Format specifications and conventions                |

#### `principle` - Principle Rules

Universal software principles and best practices for optimal code quality. These are language-agnostic design principles that guide all implementation decisions.

The `principle-*` prefix is reserved for them, and they follow the `code-principle.md` template. A rule that
only holds for one language, one layer, or one dependency is not a principle.

#### `core` - Core Rules

Fundamental coding standards applicable across the entire codebase: how errors are handled and reported, how
modules and imports are laid out, how code is documented, how tests are organized, how logging is written.
Most rules are `core`.

#### `arch` - Architectural Rules

High-level organizational and structural rules — the shape of the workspace, a crate's manifest, the
structure a service follows. An `arch` rule governs where code lives rather than how it is written.

#### `crate` - Crate-Specific Rules

Rules scoped to individual crates, using the `crate-` prefix followed by the package name. Packages here carry
no workspace prefix, so there is nothing to strip: the doc governing `nx-object` is `crate-nx-object`. A
security companion takes the same name plus `-security`, and the `scope` field uses the same package name
(`crate:nx-object`).

A document governing a family of sibling crates names the family, not one member.

Reach for this type only when a rule genuinely cannot generalize; a fact about a single module belongs in that
module, not in `docs/code/` ([§1](#1-core-principles)). No `crate-*` document exists today; the first one is
created by writing it.

#### `meta` - Meta Rules

Documentation format specifications — this document and the per-kind templates that extend it. Meta rules
live in `docs/__meta__/`, not `docs/code/`, and are the only type that may reference each other.

### Description Guidelines

Write descriptions optimized for dynamic discovery. Unlike skills (which are executed), rule documents are loaded to guide implementation. Your description must answer two questions:

1. **What does this document explain?** - List specific rules or concepts covered
2. **When should an agent load it?** - Include trigger terms via a "Load when" clause

**Requirements:**
- Written in third person (no "I" or "you")
- Include a "Load when" clause with trigger conditions
- Be specific - avoid vague words like "overview", "various", "handles"
- No ending period

**Examples:**
- ✅ `"Modern module organization without mod.rs. Load when creating modules or organizing Rust code"`
- ✅ `"Error handling patterns, unwrap/expect prohibition. Load when handling errors or dealing with Result/Option types"`
- ✅ `"NRO segment and asset-section layout. Load when working on the NRO writer"`
- ❌ `"Module organization patterns"` (missing "Load when" trigger)
- ❌ `"This document describes error handling"` (too verbose, missing trigger)
- ❌ `"Rules for testing"` (too vague, missing trigger)

### Discovery Command

The discovery command extracts the frontmatter of every rule document for lazy loading.

**Primary Method**: Use the Grep tool with multiline mode:
- **Pattern**: `^---\n[\s\S]*?\n---`
- **Path**: `docs/code/`
- **multiline**: `true`
- **output_mode**: `content`

**Fallback**: Bash command for manual use:
```bash
grep -Pzo '(?s)^---\n.*?\n---' docs/code/*.md 2>/dev/null | tr '\0' '\n'
```

**Cross-platform alternative** (macOS compatible):
```bash
awk '/^---$/{p=!p; print; next} p' docs/code/*.md
```

---

## 3. Naming Schema

**Principle:** prefix = group. Files sharing the same first kebab-case segment form a discoverable group.

**Format:** `<prefix>-<aspect>.md`

### Group Shape

**This document does not list the rule documents.** The corpus is discovered by reading frontmatter (see
[§2](#2-frontmatter-requirements)); an inventory here would be a second source of truth that goes stale on the
first rename, and nothing would fail when it did.

What the schema fixes is the **shape**. A group is a prefix, a member adds one segment of specificity, and a
member that specializes another adds a further segment:

```
<prefix>-*                   # the group: everything sharing a first segment
├── <prefix>-<aspect>        # a member of the group
│   └── <prefix>-<aspect>-<facet>   # a member that specializes its parent
└── <prefix>-<aspect>
```

Groups in use today include `principle-*` (universal principles), `pattern-*` (design patterns), `rust-*`
(language conventions), `rust-errors-*`, `rust-mods-*`, `logging-*`, and `test-*`. A `crate-*` group
(crate-scoped rules) is reserved and currently empty. A rule document that fits none of them is standalone,
and a new group is created by writing its first member.

Two of those prefixes read alike and are answered by asking **how many crates the rule is about**:

| Prefix    | The rule is about                                    | Example subject                       |
|-----------|------------------------------------------------------|---------------------------------------|
| `rust-*`  | The language and its tooling, in any crate anywhere  | How a `Cargo.toml` is written         |
| `crate-*` | One crate, or one family of sibling crates           | How that crate's packaging is written |

So a rule that holds for a crate in any Rust project is `rust-*`, and a rule that stops being true outside one
crate is `crate-*`. A rule that holds only because *this* crate is laid out the way it is — which layer owns
what, which items sit behind `filesystem-support` — is a `crate-*` document, not a `rust-*` one.

Specialization nests by name, not by directory: `rust-mods-members` refines `rust-mods`, and
`rust-docs-comments` refines `rust-docs`. The parent is a rule document with its own content, never a router
to its children.

### Naming Rules

1. **Use kebab-case** - All lowercase, words separated by hyphens
2. **Prefix = group** - Shared first segment = same group
3. **Progressively specific** - Add specificity per segment
4. **Match filename** - `name` in frontmatter MUST match filename (minus `.md`)
5. **Flat directory** - All files at `docs/code/` root (no subdirectories)
6. **Crate patterns** - Use `crate-` prefix followed by the package name, verbatim (`crate-nx-object`);
   packages here carry no workspace prefix to strip

### Benefits

- **Discoverable** - Searching a prefix finds all related rule documents
- **Grouped** - Related documents sort together alphabetically
- **Scalable** - Easy to add new documents within a group
- **Organized** - Natural grouping when listing files

---

## 4. Cross-Reference Rules

Rule documents may reference other rule documents to establish relationships. Cross-references use defined relationship types and follow directional rules based on document type.

### Relationship Types

| Type | Meaning | Example |
|---|---|---|
| `Related` | Sibling in same prefix group | test-files <-> test-functions |
| `Foundation` | Core rule a crate/arch rule builds on | crate-nx-object -> rust-errors-reporting |
| `Companion` | Paired doc for same crate | crate-nx-object <-> crate-nx-object-security |
| `Extends` | Specializes/refines another rule document | rust-mods-members -> rust-mods |

### Direction Rules

| From Type | Can Link To |
|---|---|
| `principle` | Other principle patterns (`Related`) |
| `core` | Principle patterns (`Foundation`), other core patterns (`Related`, `Extends`) |
| `arch` | Principle/core patterns (`Foundation`), other arch patterns (`Related`) |
| `crate` | Principle/core/arch patterns (`Foundation`), own companion (`Companion`), a crate doc it specializes (`Extends`) |
| `meta` | Other meta rules only (`Extends`) |

**Key principles:**
- Principle rules are standalone and link laterally to other principle rules
- Core rules link laterally to related or parent core rules, and may reference principle rules as foundation
- Arch rules reference the principle/core rules they build on
- Crate rules reference the principle/core/arch rules they depend on, plus a companion or the crate doc they specialize
- Meta rules only reference the base format spec they extend

### References Section Format

```markdown
## References
- [rust-mods](rust-mods.md) - Extends: Base module organization
- [rust-errors-reporting](rust-errors-reporting.md) - Foundation: Error type patterns
- [crate-nx-object-security](crate-nx-object-security.md) - Companion: Security checklist
```

### Examples

- ✅ `rust-mods-members` -> `rust-mods` (Extends: core to core)
- ✅ `crate-nx-object` -> `rust-errors-reporting` (Foundation: crate to core)
- ✅ `crate-nx-object-romfs` -> `crate-nx-object` (Extends: crate to crate)
- ✅ `crate-nx-object` <-> `crate-nx-object-security` (Companion: bidirectional)
- ✅ `rust-crates` -> `principle-information-hiding` (Foundation: arch to principle)
- ✅ `test-files` <-> `test-functions` (Related: core siblings)
- ❌ `code` -> `rust-mods` (meta rules only reference other meta rules)
- ❌ `rust-mods` -> `crate-nx-object` (core cannot reference crate rules)

---

## 5. Document Structure

### Required Sections

Every rule document should follow this general structure:

| Section | Required | Description |
|---------|:--------:|-------------|
| H1 Title | Yes | Human-readable document title |
| Scope line | Optional | Bold line naming what the document governs; written only where `scope` cannot say it |
| Main content sections | Yes | Rule content organized by topic |
| Checklist | Yes | Verification checklist for rule compliance |
| References | No | Cross-references to related rule documents (follow type rules) |
| External References | No | Links to external articles, books, or specs |

**The order above is the order on the page.** The Checklist is the last thing a reader *does* in the
document; References and External References are navigation away from it, and they are a pair that stays
adjacent. A document that puts References before the Checklist splits that pair the moment it gains an
External References section.

### The Scope Line Is Written Only When It Narrows

A bold line under the H1 is warranted **only when it says something the `scope` field cannot**. Every rule
document is mandatory — [§1](#1-core-principles) establishes that for the whole corpus — so a line announcing
that a `scope: "global"` document applies to all code states three facts already stated by the frontmatter,
the title, and the corpus's own authority. It is ceremony, and ceremony drifts: it is not read, so nobody
notices when it stops matching.

Write the line when the document governs **less than its `scope` implies** — a kind of item, a file type, a
subset of crates, a decision point:

```markdown
✅ Governs something `scope` cannot express
**MANDATORY for ALL `Cargo.toml` files in the workspace**
**MANDATORY for ALL integration test files (`tests/*.rs`)**
**MANDATORY for ALL workspace-level manifest organization in the workspace**

❌ Restates `scope: "global"` and the corpus-wide mandate
**MANDATORY for ALL Rust code in the workspace**
**MANDATORY for ALL code in the workspace**
```

Where the line is written, the scope phrase is **"in the workspace"**, never a product name: a product name
carries no scope information and does not survive a rename. Where it is not written, the H1 and the `scope`
field carry the whole answer.

### Optional Sections

Include when relevant:

- **Table of Contents** - For lengthy documents
- **Complete Examples** - Comprehensive usage examples
- **Configuration** - Setup and configuration guidance

**CRITICAL**: No empty sections allowed. If you include a section header, it must have content. Omit optional sections entirely rather than leaving them empty.

---

## 6. Content Guidelines

### Code Examples Are Illustrations, Not Citations

**Every example is fabricated, and no example cites a module.** An example exists to _illustrate_ the
rule the doc states — the least code that carries the convention, invented for the purpose, standing
on its own.

This is deliberate, and it is the opposite of what a citation buys. A `// ✅ Good — nx-object/src/nro.rs`
attribution makes a doc feel checkable, but it is a **copy of a module living in a second file**, and
it rots exactly like any other copy: the crate is renamed, the helper moves, the signature grows an
argument, the code the doc quotes is deleted — and now the rule document is wrong about the repository
in a way nobody notices, because nobody re-reads a doc when they edit the code it quotes.

A fabricated example cannot drift, because it makes no claim about the codebase. It says "here is
what the rule looks like", not "here is where the rule lives" — and only the first of those is the
doc's job.

So:

- **Never write a file path into an example**, in the `// ✅ Good —` comment or anywhere else. The
  comment says _why_ the example is good or bad, never _where_ it came from.
- **Never assert, in prose, that a named module does the thing.** "`nx-object/src/nro.rs`
  states X" is a citation wearing a sentence, and it rots on the next rename. State the rule.
- **Invent the names.** Illustrative subjects (`SegmentHeader`, `BuildOptions`, `parse_header`) are
  preferred precisely because they are obviously not an inventory of the workspace.
- **Stay as close to the real code as the rule allows.** Fabricated does not mean generic. An example should
  look like something this workspace would plausibly contain — the same domain vocabulary, the same shapes,
  the same error types, the same async style — so a reader recognizes their own code in it. `foo`/`bar` and
  toy domains (shapes, animals, a restaurant) teach nothing, because the reader has to translate before they
  can apply the rule, and the translation is where the rule gets lost. Write the example you would have
  written had you been solving the real problem, then rename everything.
Three things stay exact, because they are what the doc is teaching rather than evidence for it:

- **Third-party and std APIs**: `thiserror`, `tracing`, `tokio`, `std::sync::Arc`. A doc that gets
  these wrong teaches the wrong thing.
- **The crate's own name and the format names it deals in.** `nx-object`, `NRO`, `NSO`, `RomFS`, `NACP` —
  written as they really are, never disguised. These are the project's vocabulary, and a reader who cannot map
  an example onto the format it concerns has to translate before they can apply the rule, which is the same
  cost a toy domain imposes. Invented substitutes are at their worst in a doc whose subject **is** naming,
  where the fabrication defeats the lesson. What must not follow the name is the crate's **API**: do not
  import its types or reproduce its signatures, because those drift and the name does not.
- **A layer or format a rule names as its subject.** A rule that says "every `write` builder returns its image
  as a byte buffer" is stating the convention, not citing a module. The test is whether the name is the
  **rule** or the **proof**. Evidence rots; a rule is what the reader came for.

A fabricated example is still written in this workspace's stack and style: it compiles under the workspace
lints, and it never demonstrates tooling the repository does not use.

Naming a **path pattern** is not a citation and stays allowed, because it is the convention itself:
`src/lib.rs`, `<member>/src/`, `tests/*.rs`. What is banned is pointing at one real module as
evidence.

The `Good` / `Bad` pair still carries the argument. A **Bad** example is the mistake the rule exists
to prevent, and it is at its strongest when it names the cost concretely — "this panicked on every
empty partition and no test noticed" teaches more than a bare `foo`. Invent the war story if you
must, but keep it specific: the point is the failure mode, not the provenance.

**A convention must still exist in a workspace member to be documented** ([§1](#1-core-principles)) — that
requirement is unchanged, and it is on the _author_ to have verified it. What changed is that the doc
no longer proves it by quoting a file, because that proof expires.

### Examples Are Labelled With One of Three Markers

Every example opens with a comment naming its verdict, using one of exactly three markers:

| Marker | Means                                                                 |
|--------|------------------------------------------------------------------------|
| `// ✅ Good —` | This is the form to write                                       |
| `// ❌ Bad —`  | This is the mistake the rule exists to prevent                  |
| `// 🔶 Acceptable —` | Permitted under a stated condition, not the default       |

The emoji is the point: it is scannable in a long document, survives being pasted into a review comment, and
reads at a glance in a rendered page where a bare `// Good` disappears into the code. Three markers is the
whole vocabulary — no fourth verdict, and no `CORRECT`/`WRONG` shouting.

The em dash is followed by the reason, never a restatement of the verdict: `// ❌ Bad — this truncates on any
dataset past four billion rows` says something; `// ❌ Bad — wrong` does not. What that clause must contain is
governed above: the failure mode and what it cost, never where the code came from.

### DO

- Keep rules focused and actionable
- Label every example `// ✅ Good —`, `// ❌ Bad —`, or `// 🔶 Acceptable —`
- Include code snippets showing correct and incorrect usage, in Rust
- Fabricate every example: the least invented code that carries the convention
- Verify a real module demonstrates the convention before documenting it — then write the example from scratch
- Name path patterns (`src/lib.rs`, `tests/*.rs`) where the convention is about layout
- Use consistent terminology throughout
- Include a verification checklist at the end
- Explain the reasoning behind rules

### DON'T

- Restate a rule that another document already owns (link instead)
- Restate a single module's contract (document it in that module instead)
- Cite a module in an example, or point at one in prose as evidence (it is a copy, and it drifts on the next rename)
- Transcribe real code into an example, verbatim or lightly edited
- Document a convention no workspace member demonstrates (a convention nothing demonstrates is not one)
- File a rule under a prefix whose subject it is not
- Cover more than one responsibility in a doc, or add a doc that only routes to its siblings
- Narrate a migration, or argue the case for a decision already made
- Record dependency versions, release/beta status, benchmark figures, or tool install state
- Inventory every site a rule applies to (cite the clearest example instead)
- Include project-specific business logic
- Hardcode paths that may change frequently
- Add speculative or planned rules
- Document tooling the workspace does not use
- Use vague descriptions ("various", "multiple", "etc.")
- Leave optional sections empty (omit them instead)

---

## 7. Template

Use this template when creating new rule documents:

````markdown
---
name: "{{rule-name-kebab-case}}"
description: "{{Brief summary. Load when [trigger conditions], no period}}"
type: "{{principle|core|arch|crate|meta}}"
scope: "{{global or crate:<name>}}"
---

# {{Document Title - Human Readable}}

**MANDATORY for {{what this governs}}** {{OPTIONAL - only where `scope` cannot say it; omit otherwise}}

## Table of Contents {{OPTIONAL - for lengthy documents}}

1. [Section Name](#section-name)
2. [Another Section](#another-section)
3. [Checklist](#checklist)

## {{Main Content Sections}}

{{Rule content organized by topic.
Include code examples showing correct and incorrect usage.
Every example is fabricated; name no module.}}

### {{Subsection}}

{{Detailed guidance with examples:}}

```rust
// ❌ Bad — {{what went wrong, and what it cost when it shipped}}
{{incorrect_example()}}
```

```rust
// ✅ Good — {{why this is right, and what it prevents}}
{{correct_example()}}
```

The Good example is **fabricated**: it shows the least invented code that carries the convention, and
it names no module ([§6](#6-content-guidelines)). The comment says why, never where.

## Checklist

Before committing code, verify:

- [ ] {{Verification item 1}}
- [ ] {{Verification item 2}}
- [ ] {{Verification item 3}}

## References {{OPTIONAL - follow cross-reference rules}}

- [rule-name](rule-name.md) - Relationship: Brief description

## External References {{OPTIONAL}}

- [{{External reference title}}]({{url}})
````

The template emits `## Checklist`, then `## References`, then `## External References` — the order
[§5](#5-document-structure) fixes. This ordering is **settled**: every rule document in `docs/code/`
follows it, and the per-kind templates in `docs/__meta__/` inherit it rather than restating it.

---

## 8. Checklist

Before committing a rule document:

### Frontmatter

- [ ] Valid YAML frontmatter with opening and closing `---`
- [ ] `name` is kebab-case and matches filename (minus .md)
- [ ] `type` is one of: `principle`, `core`, `arch`, `crate`, `meta`
- [ ] `scope` is valid: `global` or `crate:<name>`
- [ ] `description` includes "Load when" trigger clause (no ending period)
- [ ] Frontmatter is valid YAML (no syntax errors)

### Structure

- [ ] H1 title (human readable) after frontmatter
- [ ] A scope line appears only if it narrows what `scope` already says; if present, it reads "in the
      workspace" rather than naming a product
- [ ] Main content sections with rule details
- [ ] Checklist section for verification
- [ ] `## Checklist` precedes `## References`, which precedes `## External References`
- [ ] No empty sections (omit optional sections rather than leaving them empty)

### Naming and Organization

- [ ] File located at `docs/code/` root (no subdirectories)
- [ ] Filename uses kebab-case
- [ ] Filename uses appropriate prefix for its group
- [ ] Related documents share the same prefix
- [ ] Crate-specific documents follow `crate-<package-name>.md` format
- [ ] Internal cross-references use correct paths

### Cross-References

- [ ] References use defined relationship types (`Related`, `Foundation`, `Companion`, `Extends`)
- [ ] Crate rules reference foundation core rules
- [ ] Security companions are bidirectionally linked
- [ ] Meta rules only reference other meta rules

### Content

- [ ] Every convention documented is demonstrated by a workspace member (the author checked; the doc does not cite it)
- [ ] Code examples are Rust and compile under the workspace lints
- [ ] Every example is fabricated — no example cites a module, and no prose points at one as evidence
- [ ] Every example is labelled `// ✅ Good —`, `// ❌ Bad —`, or `// 🔶 Acceptable —`, with the reason after the dash
- [ ] No example is a transcription of real code, and a rename anywhere in the workspace could not falsify the doc
- [ ] Examples use this workspace's domain vocabulary and idioms, not toy domains a reader must translate
- [ ] The doc states rules and shows shapes; it does not enumerate the documents that exist
- [ ] Path patterns (`src/lib.rs`, `tests/*.rs`) appear only where the convention is about layout
- [ ] No rule assumes tooling the workspace does not have

### Responsibility and Durability

- [ ] The doc has one responsibility — a single reason to change
- [ ] Every rule in it is about the subject its prefix names, not merely something that touches it
- [ ] Every rule generalizes across the workspace; a fact about one module or seam is documented in that module
- [ ] Every rule in it has exactly one home; siblings are linked, not restated
- [ ] The doc carries rule content (it is not a routing index for its group)
- [ ] Rules are stated in the imperative present, not as migration narrative or as the case for a past decision
- [ ] No dependency version, beta/release status, benchmark figure, or tool install state appears
- [ ] Conventions cite an example rather than enumerating every site they apply to

### Discovery

- [ ] Description is optimized for AI agent discovery
- [ ] The document can be found via Grep multiline pattern
- [ ] Trigger conditions are clear and specific

### Review

Use the `/docs-fmt-check` skill to validate rule documents before committing.
