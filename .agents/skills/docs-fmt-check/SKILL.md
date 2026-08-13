---
name: docs-fmt-check
description: Validate rule document format against its specification. Use when reviewing PRs, after editing docs in docs/code/, or before commits
---

# Doc Format Check Skill

This skill validates that documentation **format** follows the established patterns for the code rules in
`docs/code/`.

## When to Use This Skill

Use this skill when:
- Reviewing a PR that includes rule document changes (`docs/code/`)
- After creating or editing a rule document
- Before committing changes to `docs/code/`
- User requests a doc format review

## Review Process

### Step 1: Identify Changed Docs

For recent commits:
```bash
git diff --name-only HEAD~1 | grep 'docs/code/.*\.md$'
```

For staged changes:
```bash
git diff --cached --name-only | grep 'docs/code/.*\.md$'
```

For unstaged changes:
```bash
git diff --name-only | grep 'docs/code/.*\.md$'
```

### Step 2: Validate Each Doc

For each changed doc, verify:
1. Frontmatter format
2. Content structure
3. Discovery compatibility

## Format Reference

Every `docs/code/*.md` document is validated against
[docs/__meta__/code.md](../../../docs/__meta__/code.md):

- Frontmatter fields: `name`, `description`, `type`, `scope`
- Description rules ("Load when" triggers, discovery optimization)
- Pattern type definitions (`principle`, `core`, `arch`, `crate`, `meta`)
- Scope format rules (`global`, `crate:<name>`)
- Naming conventions (kebab-case, flattened crate-specific rule documents)
- Required and optional sections, and their order on the page: main content → `Checklist` → `References` →
  `External References`
- Cross-reference rules (relationship types and direction rules)
- No subdirectories rule (all files at `docs/code/` root)

Then read the structure template for the document's group, and validate against it as well:

- `principle-*` → [docs/__meta__/code-principle.md](../../../docs/__meta__/code-principle.md) — required sections (Rule, Examples, Why It Matters, Pragmatism Caveat, Checklist), example format, header format
- `pattern-*` → [docs/__meta__/code-pattern.md](../../../docs/__meta__/code-pattern.md) — same section set, applicability line, Bad-before-Good pairs presented as a numbered list
- `rust-*` → [docs/__meta__/code-rust.md](../../../docs/__meta__/code-rust.md) — numbered content sections, doctrine paragraph, applicability line, checklist lead-in, ownership-pointer phrasing

### Discovery Validation

Verify frontmatter is extractable:

**Primary Method**: Use the Grep tool with multiline mode:
- **Pattern**: `^---\n[\s\S]*?\n---`
- **Path**: path to the changed doc
- **multiline**: `true`
- **output_mode**: `content`

**Fallback**: Bash command:
```bash
grep -Pzo '(?s)^---\n.*?\n---' <path-to-doc>
```

**Cross-platform alternative** (macOS compatible):
```bash
awk '/^---$/{p=!p; print; next} p' <path-to-doc>
```

## Validation Process

1. **Identify changed files** using `git diff` above
2. **Read the doc**, `docs/__meta__/code.md`, and the group's structure template
3. **Validate** using the checklist in the meta files
4. **Report** findings using format below

## Review Report Format

After validation, provide a structured report listing issues found.

```markdown
## Doc Format Review: <filename>

### Issues Found
1. <issue description with line number>
2. <issue description with line number>

### Verdict: PASS/FAIL

<If FAIL, provide specific fixes needed referencing the appropriate meta file>
```

## Common Issues

- Invalid frontmatter YAML syntax
- `name` not in kebab-case or doesn't match filename (minus `.md`)
- `description` missing "Load when" trigger clause
- `type` not one of: `principle`, `core`, `arch`, `crate`, `meta`
- `scope` invalid format (not `global` or `crate:<name>`)
- Crate-specific rules not following `crate-<name>.md` naming
- Rule documents in subdirectories (should be flat at `docs/code/` root)
- Sections out of order (main content → `Checklist` → `References` → `External References`)
- Missing required sections (main content, Checklist)
- Empty optional sections

## Pre-approved Commands

These tools/commands can run without user permission:
- Discovery command (Grep tool or bash fallback) on `docs/code/`
- All `git diff` and `git status` read-only commands
- Reading files via Read tool

## Next Steps

After format review:

1. **If format issues found** - List specific fixes needed
2. **If format passes** - Approve for commit
3. **Verify discovery** - Ensure frontmatter is extractable with Grep tool
