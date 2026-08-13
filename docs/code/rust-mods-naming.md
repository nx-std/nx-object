---
name: "rust-mods-naming"
description: "How a module is named: for the purpose it serves, never for an item kind (`types`, `error`), a mechanism, or nothing at all (`common`, `util`), and the subject split that repairs one. Load when naming a new module, reaching for a bucket to put an item in, or splitting a module that has grown"
type: "core"
scope: "global"
---

# Module Names

**A module's name states the purpose it serves.** A reader who knows what they are looking for knows which
file to open, and a reader who opens the file recognizes the name from what is inside it. The invariant
behind this is stated in [rust-mods](rust-mods.md): a module's name and its location are the same fact.

This document owns how that name is chosen. Where module files sit is owned by
[rust-mods-files](rust-mods-files.md), the order of items inside one by
[rust-mods-members](rust-mods-members.md), and which references between them are legal by
[rust-mods-graph](rust-mods-graph.md).

## 1. A Name States a Purpose, Not a Mechanism

A module is named for what it is for, in the vocabulary of the problem it solves — not for the data structure
or technique that happens to implement it today.

The purpose is the stable fact and the mechanism is the volatile one, so a name taken from the mechanism
starts accurate and becomes a lie on the first rewrite. Nobody renames a module when they replace what is
inside it, because the rename touches every import; so the wrong name survives, and the next reader trusts
it. A name taken from the purpose survives the rewrite that a name taken from the mechanism cannot.

The same test applies when a module's contents drift away from its name. One of the two is wrong: either the
items belong somewhere else, or the name has stopped describing what the module is for. Both are fixed by
deciding what the module's purpose actually is, and neither is fixed by leaving it.

```rust
// ❌ Bad — named for the data structure. When the free-page search moved from a bitmap to a
// size-bucketed free list, `bitmap` described nothing in the file, and every caller still
// said `bitmap::reserve` while reserving through a list.
use crate::bitmap::{
    release,
    reserve,
};
```

```rust
// ✅ Good — named for what callers come here to do. The same import is still correct after
// the search is rewritten, because reserving pages is what the module is for either way.
use crate::reservation::{
    release,
    reserve,
};
```

## 2. A Module Is Named for Its Subject, Not Its Item Kind

A module is named for the thing it is about: `savedata`, `page`, `session`, `permission`. Names that
partition by item kind — `types`, `structs`, `enums`, `traits`, `constants`, `error` — are prohibited.

An item-kind name is not a name, it is a tautology: nearly every module in the workspace contains types, so
`types` distinguishes nothing and gives a reader no way to guess where an item lives. Worse, it attracts. A
module called `savedata` refuses an unrelated gamecard struct on sight, because the name argues against it. A
module called `types` accepts it, and accepts the next one, until the file is a catalogue that everything
else imports and nothing else can be read without.

The kind split also guarantees the reference pattern [rust-mods-graph](rust-mods-graph.md) exists to prevent.
A module holding every type is imported by every module holding functions, so the graph collapses to a hub,
and the one module a reader must understand first is the one with no subject to understand.

```
// ❌ Bad — nothing predicts which file holds a save-data attribute, and `types` is imported
// by every other module here, so it must be read before any of them.
src/
  types.rs        // four unrelated subjects, grouped by all being types
  error.rs        // every error the crate returns, each far from what returns it
  constants.rs
  session.rs
```

```
// ✅ Good — the name is the index. An item's module follows from what it is about, and a
// reader who only cares about save data reads one file.
src/
  savedata.rs
  gamecard.rs
  storage.rs
  session.rs
```

Why an error type is declared beside the function that returns it, rather than gathered into a module of its
own, is owned by [rust-errors-reporting](rust-errors-reporting.md). This document owns only the naming half:
`error` is an item kind, so it is not a module name.

## 3. A Module Named for Nothing Collects Everything

`common`, `core`, `util`, `utils`, `helpers`, `misc`, `shared`, and `base` are prohibited as module names.

A name that does not name a subject supplies no test for what belongs in it, so nothing can ever be argued
out. Every one of these starts as two genuinely shared helpers and ends as the module with the most reasons
to change in the crate, because the only membership rule is that the author could not think of anywhere
else. The name records that failure and then perpetuates it: the next author, equally stuck, finds a module
that will take anything.

The absence of a subject is also the absence of a boundary. A crate can be reorganized around subjects; it
cannot be reorganized around `misc`, because the split that would fix it is exactly the thinking the name let
everybody skip. `core` carries a second cost of its own — it shadows the `core` crate every `no_std` module
in the workspace imports, so `use core::…` and `use crate::core::…` sit two characters apart in the same
file.

When the shared items do have a subject, name it. When they do not, they are usually not one module:
`util` holding a path joiner and a retry policy is two modules whose only relation was that neither had an
obvious home.

```rust
// ❌ Bad — `common` has no membership rule, so it grew a path joiner, a result-code
// converter and a spin loop. The crate now has one module every other module imports and
// no two items in it change for the same reason.
use crate::common::{
    join_path,
    spin_hint,
    to_result_code,
};
```

```rust
// ✅ Good — three subjects, three modules, each importable on its own. A reader chasing a
// result code never compiles the spin loop.
use crate::{
    path::join_path,
    result::to_result_code,
    spin::spin_hint,
};
```

## 4. A Bucket Is Split by Subject, Not by Size

A module that has grown is split along what its items are about. Splitting it by size, by item kind, or into
a remainder module keeps the original defect and adds a second file.

The test is whether each half can be read alone. A subject split produces modules that stand on their own,
and it usually removes the references between them: when the items one subject owns move together, the types
they share move with them, and two modules that pointed at each other stop doing so. A size split guarantees
the opposite, because it cuts through the middle of a subject and leaves every seam it crossed as an import.

A remainder is the same mistake wearing a smaller file. Splitting `types` into `savedata` plus a `types` that
holds what was left over does not produce a subject and a subject; it produces a subject and a bucket, and
the bucket keeps attracting. Extracting one subject at a time is a legitimate way to drain a bucket, and the
subject under active work is the right one to take first — but until the last item has landed somewhere named
for what it is about, the remainder is still a violation of
[§2](#2-a-module-is-named-for-its-subject-not-its-item-kind), not a module that has been made smaller.

Watch the shared item that both halves reach for. It belongs with whichever subject genuinely owns it, and
moving it there is what makes the split acyclic; leaving it behind in the remainder is what forces the two
modules to reference each other, which [rust-mods-graph](rust-mods-graph.md) prohibits.

```
// ❌ Bad — split on line count. Neither half names a subject, and the account id left in
// the second is reached by every struct in the first, so the two now import each other and
// neither can be read or moved alone.
src/
  types_a.rs
  types_b.rs
```

```
// ✅ Good — split on subject, and the account id moved to the module that keys things by
// it. Neither module refers to the other, so either can be read first.
src/
  savedata.rs     // the save kinds, the attribute, and the account id they are keyed by
  gamecard.rs
```

## Checklist

Before committing code, verify:

- [ ] Every module name states the purpose the module serves, not the data structure or technique
      implementing it
- [ ] No module's name has been left behind by what the module now contains
- [ ] No module is named for an item kind (`types`, `structs`, `enums`, `traits`, `constants`, `error`)
- [ ] Every module name names a subject a reader could guess an item's location from
- [ ] No module is named `common`, `core`, `util`, `utils`, `helpers`, `misc`, `shared`, or `base`
- [ ] No module exists whose membership rule is "had nowhere else to go"
- [ ] A split module was cut along subjects, not line count, item kind, or a remainder
- [ ] An item both halves of a split reach for was moved into the subject that owns it, leaving no
      reference between the two new modules

## References

- [rust-mods](rust-mods.md) - Extends: The invariant that a module's name and its location are the same fact
- [rust-mods-files](rust-mods-files.md) - Related: Where a module file sits once it has a name
- [rust-mods-graph](rust-mods-graph.md) - Related: Which references between modules a subject split must not
  leave behind
- [rust-mods-members](rust-mods-members.md) - Related: The order items take inside a module, once it holds
  only one subject
- [rust-errors-reporting](rust-errors-reporting.md) - Related: Owns why an error type is declared beside the
  function that returns it
- [principle-single-responsibility](principle-single-responsibility.md) - Foundation: A module named for an
  item kind has as many reasons to change as it has items
