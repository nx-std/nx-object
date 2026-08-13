---
name: "rust-mods"
description: "Module doctrine: a module is named by its path, a parent declares its children, references flow one way. Load when adding a module, moving one, or deciding which file an item belongs in"
type: core
scope: "global"
---

# Rust Module Doctrine

**MANDATORY for ALL Rust code in this workspace**

A crate's module tree is its structure. Three invariants hold everywhere in the workspace, and each rule in
the `rust-mods-*` group is one of them made operational.

## 1. A Module Is Found by Its Name

A module's name and its location are the same fact. File and directory names match the module name exactly,
so a reader who knows the name knows which file to open, and a stack frame names the module it came from.

Where a module lives is never a convention learned separately from the module itself.

## 2. A Parent Declares Its Children and Owns What Escapes Them

The unit of encapsulation is the module, not the type. A parent declares which children exist and re-exports
what callers may reach, so a child's internals stay a decision the parent can revise without any caller
learning that it did.

It follows that a module's public surface is what its parent chose to re-export, never the union of
everything its descendants happened to mark `pub`.

## 3. References Flow One Way

A module refers to what it owns and to what sits beside it, never back to the file that declared it. The
reference graph over a crate's modules is acyclic: down to a child, or across to a sibling, with no path
returning to where it started.

A tree whose edges point both ways is not a hierarchy, it is a graph drawn to look like one, and it costs a
reader the ability to understand any subtree without first reading its parent. When a child needs something
its parent holds, the item is in the wrong file, and moving it is the fix.

## Checklist

Before adding or moving a module, verify:

- [ ] The module's file and directory names match its name exactly
- [ ] The parent declares the child and re-exports what callers outside the module need
- [ ] Nothing the module needs is declared in the file that declares the module
- [ ] No chain of references leads from a module back to that same module

## References

- [rust-mods-naming](rust-mods-naming.md) - Related: How a module's name is chosen, and the subject split that repairs one that was not
- [rust-mods-files](rust-mods-files.md) - Related: Where module files sit and what the named module file carries
- [rust-mods-graph](rust-mods-graph.md) - Related: Which references between those files are legal
- [rust-mods-members](rust-mods-members.md) - Related: Order of the items inside a single module file
- [principle-information-hiding](principle-information-hiding.md) - Foundation: The module is the privacy boundary, and visibility is gated at the declaration
