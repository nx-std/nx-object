# nx-std/nx-object - Technical Overview for Coding Agents

## Project Summary

`nx-std/nx-object` is a single Rust library crate for **Nintendo Switch file formats**: zero-copy parsing and
generation of the executable and asset formats the console loads.

The crate is **`no_std` by default and `std` on demand**. Nothing in `raw` or `read` needs an allocator or an
operating system, so the same definitions serve host-side tooling and code running on the console; the `write`
layer and everything that touches a filesystem sit behind the `filesystem-support` feature.

## Quick Start

**If you're an AI agent working on this codebase, here's what you need to know immediately:**

1. **Invoke `/code-rules` FIRST** → Before planning OR coding (applies in Plan mode too), load the `docs/code/` rules that govern the change. Skipping this leads to plans that violate project conventions and cause rework.
2. **Use Skills for operations** → Invoke skills (`/code-format`, `/code-check`, `/code-test`, `/code-rules-check`, `/docs-fmt-check`, `/code-review`) instead of running commands directly.
3. **Skills wrap justfile tasks** → Skills provide the interface to `just` commands with proper guidance.
4. **Follow the workflow** → Format → Check → Clippy → Test → Rules check.
5. **Mind the feature axis** → A change outside `filesystem-support` must still compile without `std` (`just check --no-default-features --target aarch64-unknown-none`).
6. **Fix ALL warnings** → Zero tolerance for clippy warnings.

**Your first action**: Invoke `/code-rules` to load the rules before drafting a plan or writing any code. For commands, invoke the relevant Skill.

## Table of Contents

1. [Principles](#1-principles) — Core design principles guiding this codebase
2. [Code Rules](#2-code-rules) — Understanding coding standards via the `/code-rules` skill
3. [Architecture](#3-architecture) — Layers, formats, and features
4. [Build System](#4-build-system) — Cargo, prerequisites, feature configurations
5. [Development Workflow](#5-development-workflow) — How to develop with this codebase


## 1. Principles

**MANDATORY**: Before writing any code, read and internalize these core design principles. Full details, examples, and checklists are in the linked docs — read them every time.

| Principle                   | One-liner                                                        | Full doc                                              |
|-----------------------------|------------------------------------------------------------------|-------------------------------------------------------|
| **Single Responsibility**   | One struct = one reason to change                                | @docs/code/principle-single-responsibility.md         |
| **Open/Closed**             | Extend via new types/trait impls, don't modify existing code     | @docs/code/principle-open-closed.md                   |
| **Law of Demeter**          | Only talk to immediate collaborators — no `a.b().c().d()` chains | @docs/code/principle-law-of-demeter.md                |
| **Validate at the Edge**    | Hard shell (boundary validates), soft core (domain trusts)       | @docs/code/principle-validate-at-edge.md              |
| **Type-Driven Design**      | Make illegal states unrepresentable via the type system          | @docs/code/principle-type-driven-design.md            |
| **Idempotency**             | Operations safe to retry — same effect whether run once or N×    | @docs/code/principle-idempotency.md                   |
| **Inversion of Control**    | Depend on abstractions, not concretions                          | @docs/code/principle-inversion-of-control.md          |
| **Least Surprise**          | Code behaves the way readers expect from its name and signature  | @docs/code/principle-least-surprise.md                |
| **DRY/WET balance**         | Deduplicate real knowledge; tolerate incidental similarity       | @docs/code/principle-dry-wet.md                       |
| **Information Hiding**      | Expose what callers need; keep representation and helpers private| @docs/code/principle-information-hiding.md            |
| **Rate of Change**          | Group state and code by how often they change                    | @docs/code/principle-rate-of-change.md                |
| **Symmetry**                | Express one idea one way; split near-duplicates cleanly          | @docs/code/principle-symmetry.md                      |

Every `principle-*` description in the `/code-rules` catalog states its rule in full, so loading the catalog
gives you all of them. Read a full document when you need its examples or its Pragmatism Caveat to argue a
decision, or invoke `/code-rules principles` to load and summarize them all.


## 2. Code Rules

The code rules live in `docs/code/` with YAML frontmatter for dynamic discovery. "Code rules" and "code
guidelines" name the same corpus.

**Rule docs are authoritative**: They define how code should be written. All implementations MUST follow the patterns. If code doesn't follow a pattern, either fix the code or update the pattern.

### Rule Types

| Type               | Scope          | Purpose                                                          |
|--------------------|----------------|------------------------------------------------------------------|
| **Principle**      | `global`       | Universal software principles and best practices                 |
| **Core**           | `global`       | Fundamental coding standards (error handling, logging, modules)  |
| **Architectural**  | `global`       | High-level patterns (crate layout, manifest shape)               |
| **Pattern**        | `global`       | Reusable design patterns (builder, typestate)                    |
| **Crate-specific** | `crate:<name>` | Reserved; no such document exists yet                            |
| **Meta**           | `global`       | Documentation format specifications (`docs/__meta__/`)           |

### Skill Invocation

| When You Need To                                                  | Invoke This Skill   |
|-------------------------------------------------------------------|---------------------|
| Load the code rules before planning or implementing               | `/code-rules`       |
| "How should I handle errors?", "What's the pattern for X?"        | `/code-rules`       |
| Audit a finished changeset against the rules                      | `/code-rules-check` |
| Validate a rule document's own format after editing `docs/code/`  | `/docs-fmt-check`   |
| Deep review before a PR (bugs, security, soundness)               | `/code-review`      |

**Navigation:**

- Need to understand patterns? → `/code-rules`
- All rules located in `docs/code/`
- Documentation format specs in `docs/__meta__/`

### Code Style

- Uses unstable `rustfmt` features (nightly required for formatting)
- Stable toolchain (`1.97.1`) for compilation, pinned via `rust-toolchain.toml`
- Imports grouped: std, external crates, local (`group_imports = "StdExternalCrate"`)
- Import granularity at crate level (`imports_granularity = "Crate"`)


## 3. Architecture

The crate is one package with three layers, stacked in one direction: `write` and `read` both build on `raw`,
and `raw` depends on nothing above it.

| Layer   | What it is                                                                      | Availability          |
|---------|----------------------------------------------------------------------------------|-----------------------|
| `raw`   | `#[repr(C)]` binary structures backed by `zerocopy`; direct field access          | Always                |
| `read`  | Parsing wrappers that validate magic numbers and sizes, one error type per format | Always                |
| `write` | Builders that assemble a format and return the finished image as a byte buffer    | `filesystem-support`  |
| `elf`   | Derives NRO and NSO segments from a linked ELF binary                             | `elf-parsing`         |

Formats covered: NRO, NSO, KIP, NACP, NPDM, RomFS, PFS0, and the MOD0 header embedded in executables. Not every
format has all three layers; see the table in `README.md`.

### Features

| Feature              | Pulls in                          | Effect                                                     |
|----------------------|-----------------------------------|------------------------------------------------------------|
| (none, the default)  | —                                 | `no_std`; `raw` and `read` only                            |
| `filesystem-support` | `fs-err`, `sha2`, `bit_field`     | Enables `std` and the `write` layer                        |
| `elf-parsing`        | `object` (+ `filesystem-support`) | ELF segment derivation                                     |
| `lz4-compression`    | `lz4_flex` (+ `filesystem-support`)| LZ4 compression of NSO segments                           |

`#![cfg_attr(not(feature = "filesystem-support"), no_std)]` in `src/lib.rs` is what makes the default build
`no_std`. Code that is not behind a `filesystem-support` gate therefore may not reach for `std`, and the check
that proves it is `just check --no-default-features --target aarch64-unknown-none`. That target stands in
for the console's `aarch64-nintendo-switch-freestanding`, which is tier 3 and would need `-Z build-std` on
nightly; both are 64-bit aarch64, so pointer width and alignment match what the console sees.

CI runs the two configurations as a matrix, `check (std)` and `check (no-std)`, invoking the same `just`
recipes with the flags each configuration needs. The recipes take the flags rather than baking them in, so
there is one definition of each check and no way for local and CI to drift apart. `test` and `format` are
separate jobs.


## 4. Build System

A plain Cargo package driven by `just` recipes.

### Prerequisites

- Rust stable toolchain `1.97.1` (specified in `rust-toolchain.toml`) for compilation
- Rust nightly toolchain for formatting (required by unstable rustfmt features)
- `just` command runner
- Optional: `cargo-nextest` for faster test runs (`just test` falls back to `cargo test`)

### Build

```
cargo build                    # default features: no_std
cargo build --all-features     # every layer
cargo build --no-default-features   # explicit no_std build
```

Standard cargo — there is no extra build orchestration.


## 5. Development Workflow

**This section provides guidance for AI agents on how to develop with this codebase.**

### Documentation Structure: Separation of Concerns

This project uses three complementary documentation systems. Understanding their roles helps AI agents navigate efficiently:

| Documentation                  | Purpose                  | Content Focus                                                                                                                                |
|--------------------------------|--------------------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| **AGENTS.md** (this file)      | **WHY** and **WHAT**     | Project architecture, policies, goals, and principles. Answers "What is this project?" and "Why do we do things this way?"                   |
| **Skills** (`.agents/skills/`) | **HOW** and **WHEN**     | Command-line operations and just usage. Answers "How do I run commands?" and "When should I use each command?"                               |
| **Rules** (`docs/code/`)       | **HOW** (implementation) | Code implementation rules and standards (see [Code Rules](#2-code-rules)). Answers "How do I write quality, conventional code?"|

**Navigation Guide for AI Agents:**

- Need to understand the project? → Read this file (AGENTS.md)
- Need to run a command? → Invoke the appropriate Skill (`/code-format`, `/code-check`, `/code-test`)
- Need to write code? → Use `/code-rules` to load the governing rules
- Finished a change? → Use `/code-rules-check` to audit it against those rules

### Core Operating Principle

**🚨 MANDATORY: USE Skills for all common operations. Skills wrap just tasks with proper guidance.**

#### The Golden Rule

**USE Skills (`/code-format`, `/code-check`, `/code-test`, `/code-review`) for all common operations. Only use `cargo` or `just` directly when the operation is NOT covered by a skill.**

**Decision process:**

1. **First**: Check if a skill exists for your operation
2. **If exists**: Invoke the skill (provides proper flags, setup, and error handling)
3. **If not exists**: You may run the tool directly (e.g., one-off `cargo` introspection commands)

#### Why Skills Are Preferred

- **Consistency**: Uniform command execution across all developers and AI agents
- **Correctness**: Skills ensure proper flags and toolchain selection
- **Guidance**: Skills provide context on when and how to use commands
- **Pre-approved workflows**: Skills document which commands can run without user permission

#### Examples

- ✅ **Use skill**: `/code-format` (formatting Rust)
- ✅ **Use skill**: `/code-check` (compile check, clippy, `no_std` check)
- ✅ **Use skill**: `/code-test` (run cargo tests)
- ✅ **Use skill**: `/code-rules-check` (audit a changeset against the rules)
- ✅ **Use skill**: `/code-review` (deep review before a PR)
- ✅ **Direct tool OK**: `cargo tree -e features` (introspection not in justfile)

#### Command Execution Hierarchy (Priority Order)

When determining which command to run, follow this strict hierarchy:

1. **Priority 1: Skills** (`.agents/skills/`)
   - Skills are the **SINGLE SOURCE OF TRUTH** for all command execution
   - If a Skill documents a command, use it EXACTLY as shown
   - Skills override any other guidance in AGENTS.md or elsewhere

2. **Priority 2: AGENTS.md workflow**
   - High-level workflow guidance (when to format, check, test)
   - Refers you to Skills for specific commands

3. **Priority 3: Everything else**
   - Other documentation is supplementary
   - When in conflict, Skills always win

### Command-Line Operations Reference

**🚨 CRITICAL: Use skills for all operations — invoke them before running commands.**

Available skills and their purposes:

- **Formatting**: `/code-format` — Format Rust code after editing files
- **Checking/Linting**: `/code-check` — Validate compilation (including the `no_std` build) and lint with clippy
- **Testing**: `/code-test` — Run cargo tests
- **Rules**: `/code-rules` — Load the code rules and patterns that govern the work
- **Rules check**: `/code-rules-check` — Audit a changeset against those rules
- **Doc format**: `/docs-fmt-check` — Validate rule documents in `docs/code/` against their format spec
- **Reviewing**: `/code-review` — Deep review for bugs, security, soundness

### Pre-Implementation Checklist

**BEFORE drafting a plan OR writing ANY code, you MUST:**

1. **Understand the task** — Research the codebase and identify the affected layer(s) and feature gates
2. **🚨 MANDATORY: Load the code rules FIRST** — Invoke `/code-rules` before drafting any plan or writing any code. This applies equally in Plan mode: the plan itself MUST be grounded in the loaded rules, not in assumptions about conventions.
3. **Follow the matched rules** — Match the task against the catalog triggers and read the rules it names
4. **Rationale** — Skipping `/code-rules` leads to plans that use the wrong module layout, error pattern, or validation boundary, causing avoidable rework.

### Typical Development Workflow

**Follow this workflow when implementing features or fixing bugs:**

#### 1. Research Phase

- Understand the codebase and the rules that govern it
- Identify the layer the change belongs in, and whether it must hold under `no_std`
- Use `/code-rules` to load the relevant implementation rules

#### 2. Planning Phase

**🚨 MANDATORY FIRST STEP (including in Plan mode):** Invoke `/code-rules` to load the governing rules BEFORE drafting the plan. The plan's structure, module layout, error handling, and type design decisions MUST reflect the loaded rules.

- Create the implementation plan on top of the loaded rules
- Ensure plan follows required patterns (error handling, type design, module structure)
- Decide which feature gate, if any, the new code sits behind
- Consider edge cases and error handling according to the rules
- Ask user questions if requirements are unclear

#### 3. Implementation Phase

**🚨 CRITICAL: Before running ANY command in this phase, invoke the relevant Skill.**

**Copy this checklist and track your progress:**

```
Development Progress:
- [ ] Step 1: Write code following the rules (use /code-rules)
- [ ] Step 2: Format code (use /code-format skill)
- [ ] Step 3: Check compilation, including no_std when affected (use /code-check skill)
- [ ] Step 4: Fix all compilation errors
- [ ] Step 5: Run clippy (use /code-check skill)
- [ ] Step 6: Fix ALL clippy warnings
- [ ] Step 7: Run tests when warranted (use /code-test skill)
- [ ] Step 8: Audit the changeset (use /code-rules-check skill)
- [ ] Step 9: All required checks pass ✅
```

**Detailed workflow for each work chunk (and before committing):**

1. **Write code** following the rules from [Code Rules](#2-code-rules) (loaded via `/code-rules`)

2. **Format before checks/commit**:
   - **Use**: `/code-format` skill when you finish a coherent chunk of work
   - **Validation**: Verify no formatting changes remain

3. **Check compilation**:
   - **Use**: `/code-check` skill after changes
   - **Must pass**: Fix all compilation errors, in every feature configuration the change affects
   - **Validation**: Ensure zero errors before proceeding

4. **Lint with clippy**:
   - **Use**: `/code-check` skill for linting
   - **Must pass**: Fix all clippy warnings
   - **Validation**: Re-run until zero warnings before proceeding

5. **Run tests (when warranted)**:
   - **Prerequisite**: `/code-format` and `/code-check` must both be clean
   - **Use**: `/code-test` skill — runs `cargo test` (or `cargo nextest run`)
   - **When to run**: Behavior changes, public API changes, or new logic in tested code paths
   - **Validation**: Fix failures or record why tests were skipped

6. **Check the changeset against the rules**:
   - **Use**: `/code-rules-check` once the implementation is complete
   - **What it does**: selects the `docs/code/` rules the diff is governed by and reports violations with the document behind each one
   - **Validation**: Fix every finding, or record why the deviation is deliberate
   - **Doc edits**: If the change touches `docs/code/`, also run `/docs-fmt-check`

7. **Iterate**: If any validation fails → fix → return to step 2

**Visual Workflow:**

```
Edit File → /code-format skill
          ↓
    /code-check skill (compile) → Fix errors?
          ↓                            ↓ Yes
    /code-check skill (clippy) → (loop back)
          ↓
    ALL CHECKS GREEN ─ gate
          ↓
    /code-test skill (when warranted)
          ↓              ↓ Fix failure?
    All Pass ✅     (loop back to /code-format)
          ↓
    /code-rules-check skill (audit the changeset)
          ↓              ↓ Violations?
    Clean ✅        (fix, loop back to /code-format)
```

**Remember**: Invoke Skills for all operations. If unsure which skill to use, refer to the Command-Line Operations Reference above.

#### 4. Completion Phase

- Ensure all required checks pass (format, check, clippy, tests, rules check)
- If tests were skipped, document why and the risk assessment
- Use `/code-review` when the change is large or risky enough to need bugs, regressions, security, and soundness examined as well
- Document any warnings you couldn't fix and why

### Core Development Principles

**ALL AI agents MUST follow these principles:**

- **Consistency and homogeneity are fundamental**: The codebase must read as if written by a single author. All new code
  must match the style, structure, and conventions of the surrounding code. Deviating from an established pattern requires
  strong, explicit justification — "I prefer it this way" is not sufficient. If you believe a pattern should change, propose
  the change to the pattern documentation first; do not introduce one-off divergences.
- **Research → Plan → Implement**: Never jump straight to coding
- **Rules before planning**: `/code-rules` is a prerequisite for both planning AND Plan mode, not just implementation. A plan written without the loaded rules is considered incomplete.
- **Rule compliance**: Follow the rules from [Code Rules](#2-code-rules), and verify with `/code-rules-check` before committing
- **Zero tolerance for errors**: All automated checks must pass
- **Clarity over cleverness**: Choose clear, maintainable solutions

**Essential conventions:**

- **Maintain type safety**: Leverage Rust's type system fully (see [Type-Driven Design](docs/code/principle-type-driven-design.md))
- **Validate at boundaries**: Hard shell on the parsing surface, where every input is a file someone else produced; soft core inside (see [Validate at the Edge](docs/code/principle-validate-at-edge.md))
- **Keep the `no_std` half `no_std`**: Ungated code may not reach for `std`
- **Format code before checks/commit**: Use `/code-format` skill
- **Fix all warnings**: Use `/code-check` skill for clippy
- **Test behavior changes**: Use `/code-test` skill after checks are green
- **Audit before committing**: Use `/code-rules-check` skill on the finished changeset

### Summary: Key Takeaways for AI Agents

| What                | Where                                  | When                                                  |
|---------------------|----------------------------------------|-------------------------------------------------------|
| **Plan work**       | `/code-rules`                          | BEFORE creating any plan                              |
| **Run commands**    | `.agents/skills/`                      | Check Skills BEFORE any command                       |
| **Write code**      | [Code Rules](#2-code-rules)            | Load the rules before implementation                  |
| **Format**          | `/code-format`                         | Before checks or before committing                    |
| **Check**           | `/code-check`                          | After formatting                                      |
| **Lint**            | `/code-check`                          | Fix ALL warnings                                      |
| **Test**            | `/code-test`                           | After checks green; on behavior changes               |
| **Rules check**     | `/code-rules-check`                    | After tests pass; before committing                   |
| **Doc format**      | `/docs-fmt-check`                      | After editing `docs/code/`                            |
| **Review**          | `/code-review`                         | Before PRs; on large or risky changes                 |

**Golden Rules:**

1. ✅ Invoke Skills for all common operations
2. ✅ Skills wrap just tasks with proper guidance
3. ✅ Follow the workflow: Format → Check → Clippy → Test → Rules check
4. ✅ Zero tolerance for errors and warnings
5. ✅ Every change improves the codebase

**Remember**: When in doubt, invoke the appropriate Skill!
