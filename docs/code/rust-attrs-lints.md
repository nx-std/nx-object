---
name: "rust-attrs-lints"
description: "Lint suppression with #[expect] and a mandatory reason, scoped to the narrowest item. Load when silencing a compiler or clippy warning, reviewing an #[allow], or adding lint attributes"
type: "core"
scope: "global"
---

# Lint Attributes

**MANDATORY for ALL Rust code in the workspace**

## 1. Fixing Beats Suppressing

A lint fires because the code triggered a pattern the lint describes, so the first question is always whether
the lint is right. Most clippy warnings name a real simplification, and suppressing one to avoid a two-line
edit trades a permanent annotation for a temporary inconvenience. A suppression is warranted when the lint's
premise does not hold here — the "simpler" form it suggests is wrong, the complexity it flags is inherent to
the design, or the pattern is required by a trait or macro the code does not control. That claim is what the
`reason` records.

Lints that guard correctness are not suppressed to make code compile: `unwrap`/`expect` on a production path
are refused rather than silenced. See [rust-errors-handling](rust-errors-handling.md).

## 2. A Value Held Only for Its Drop Takes an Underscore

`dead_code` on a field or binding that exists for its destructor or its registration side effect is the common
case where a suppression is reached for and the language already has the answer. Rustc's `dead_code` lint
skips names beginning with `_`, so the convention says what the attribute was going to say, in the place a
reader is already looking: at the name.

```rust
// ❌ Bad — three lines of attribute to state what the name could have stated. The reader
// meets the suppression before the field, and the reason is the only thing telling them
// the field is not simply unfinished.
pub struct StdioForwarder {
    #[expect(dead_code, reason = "held only to abort the forwarding task on drop")]
    task: AbortOnDropHandle<()>,
}
```

```rust
// ✅ Good — the underscore marks it as write-only at every mention of it, and the rustdoc
// carries the part a name cannot: what dropping it does.
pub struct StdioForwarder {
    /// Owns the task draining the console's stdout, so dropping the forwarder does not
    /// leave it printing behind a finished run.
    ///
    /// Underscored because the handle is never read: holding it is what keeps the task
    /// alive, and dropping it is what stops it.
    _task: AbortOnDropHandle<()>,
}
```

The same applies to a `let` binding kept alive for a guard or a subscription: `let _guard = span.enter();`
rather than a suppression around it. A **bare** `_` is a different thing and usually a bug here — it drops the
value immediately, ending the very effect the binding exists for.

This is not licence to underscore an item to quiet the lint. `dead_code` on something genuinely unused is the
lint being right, and the fix is deleting the item or `cfg`-gating it (§6). The underscore is only for a value
whose purpose is its `Drop`, and the rustdoc has to say what that purpose is.

## 3. Suppress With `#[expect]`, Never `#[allow]`

`#[expect]` warns when the lint it names **stops** firing. `#[allow]` does not: it stays behind after the
refactor that made it unnecessary, silently disarming the lint for whatever gets written under it later. A
suppression is a claim about the code beneath it, and `#[expect]` is the form that gets checked.

```rust
// ❌ Bad — the suppression outlives its cause. This function once took nine parameters;
// it now takes four, and the attribute is still disarming the lint for the next person
// who adds five more. Nothing will ever point that out.
#[allow(clippy::too_many_arguments)]
pub fn write_nro_header(out: &mut Vec<u8>, text: &Segment, rodata: &Segment) -> Result<(), Error> {}
```

```rust
// ✅ Good — when the argument count comes down, the attribute itself becomes a warning,
// and the cleanup that made it obsolete also removes it.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the NRO header's field set; grouping them into a struct would duplicate NroHeader"
)]
pub fn write_nro_header(
    out: &mut Vec<u8>,
    text: &Segment,
    rodata: &Segment,
    data: &Segment,
    bss_size: u32,
    build_id: &BuildId,
    module_offset: u32,
) -> Result<(), Error> {}
```

## 4. Every Suppression Carries a `reason`

The `reason` states **why the lint is wrong here**, in terms of the design. It is not a restatement of the
lint's name, and it is not "clippy false positive" — every suppression author believes that.
`reason = "the type is complex"` on `clippy::type_complexity` repeats the lint: a reviewer learns nothing, and
the next person cannot tell whether the claim still holds.

```rust
// ✅ Good — the reason names the design decision the lint is arguing against, so it can
// be re-evaluated when that decision changes.
#[expect(
    clippy::type_complexity,
    reason = "the per-entry lifetimes are inherent to walking the image without copying its name table"
)]
fn walk<'a>(dir: &RomFsDir<'a>) -> Result<Vec<(EntryPath<'a>, RomFsFile<'a>)>, WalkError> {}
```

A reason that would read the same on any suppression of that lint is not a reason.

## 5. Scope It to the Narrowest Item

Put the attribute on the item that triggers the lint — the function, the field, the statement — not the module
or the crate. A module-level `#![allow(clippy::missing_errors_doc)]` disarms the lint for code that has not
been written yet, so six months later three more functions in the file are triggering it and nobody knows; at
crate level it is disarmed everywhere, permanently and invisibly.

```rust
// ✅ Good — the suppression covers exactly the item that earned it.
#[expect(clippy::missing_errors_doc, reason = "errors are documented on the trait method this implements")]
fn segment(&self, kind: SegmentKind) -> Result<&[u8], SegmentError> {}
```

The same applies to test code: a suppression needed by a test goes on the test, not on the `mod tests` block.

## 6. `#[allow]` Is Forbidden

Outside a macro body, there is no accepted use of `#[allow]` or `#![allow]` in this workspace. Every
suppression is an `#[expect]`, so one that stops being true becomes a warning instead of a silent hole.

The case that tempts people is a lint that fires **conditionally** — under one feature combination but not
another, or under `cfg(test)` but not a normal build. `#[expect]` is itself a lint and warns when the expected
lint does not fire, so a bare one would warn in exactly the configurations where the code is already clean.
That is an argument for scoping the expectation to the configuration, not for abandoning it:

```rust
// ❌ Bad — reaches for #[allow] because the lint only fires with the feature off.
// The suppression now also covers the build where the lint would have been right.
#[allow(dead_code, reason = "unused when the elf feature is off")]
struct SegmentScratch { /* ... */ }
```

```rust
// ✅ Good — the expectation is gated to the configuration that produces the lint, so it
// is still checked in that configuration and absent in the one where it does not apply.
#[cfg_attr(
    not(feature = "elf"),
    expect(dead_code, reason = "only constructed by the ELF segment extractor")
)]
struct SegmentScratch { /* ... */ }
```

If the `cfg_attr` is getting hard to write, that is usually the lint making a real point: an item dead in one
configuration often belongs behind the same `cfg` as the code that uses it, which removes the warning and the
attribute together.

### The One Exception: Inside a Macro Expansion

A `macro_rules!` body emits the same attribute for every invocation, but the lint may fire for only some of
them. An `#[expect]` there is unfulfilled — and therefore a warning — at exactly the call sites where the
generated item _is_ used. There is no `cfg` to key on: the deciding fact is what each caller does with the
expansion.

```rust
// ✅ Good — #[allow] inside the expansion, with a reason naming why the expectation
// cannot be fulfilled: the lint depends on the invocation, not on this code.
macro_rules! raw_newtype {
    ($name:ident $inner:ty) => {
        impl $name {
            #[allow(dead_code, reason = "generated accessor; not every newtype reads back")]
            pub fn get(&self) -> &$inner {
                &self.0
            }
        }
    };
}
```

This exception is confined to attributes written **inside a macro body** whose firing genuinely varies by
invocation, not to the macro's definition site, the modules that invoke it, or code that is merely near a
macro. An `#[allow]` anywhere else is a defect, including ones that predate this document: converting one to
`#[expect]` either succeeds, or fails and thereby proves the suppression was already stale.

## Checklist

Before committing code, verify:

- [ ] The lint was considered on its merits, and fixing it was rejected for a stated design reason
- [ ] A value held only for its `Drop` or registration effect is underscored, not `dead_code`-suppressed,
      and its rustdoc says what dropping it does
- [ ] The suppression uses `#[expect]`; no `#[allow]` or `#![allow]` was added outside a macro body
- [ ] The attribute carries `reason = "..."` explaining why the lint is wrong here, not what the lint is
- [ ] The reason is specific to this site; it would not read identically on any other suppression
- [ ] The attribute is on the item that triggers the lint, not on a module or the crate
- [ ] A conditionally-firing lint is handled with `cfg_attr(..., expect(...))`, not by downgrading to `#[allow]`
- [ ] Any `#[allow]` sits inside a macro expansion whose lint genuinely varies by invocation, and says so in
      its `reason`
- [ ] No correctness lint (`unwrap_used`, `panic`) is suppressed on a production path

## References

- [rust-errors-handling](rust-errors-handling.md) - Related: Correctness lints are fixed, not suppressed
- [rust-attrs-derived](rust-attrs-derived.md) - Related: The other attribute family, and its ordering rules
- [rust-docs](rust-docs.md) - Related: A `reason` is documentation, and follows the same rule about stating
  why rather than what
- [principle-least-surprise](principle-least-surprise.md) - Foundation: A suppression that outlives its cause makes
  the code behave differently from what the lint config promises
