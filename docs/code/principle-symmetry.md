---
name: "principle-symmetry"
description: "Symmetry — express the same idea the same way; split near-duplicates into identical parts and clearly different parts, one altitude per body. Load when writing something that resembles existing code, or reviewing sibling functions, branches, or modules"
type: "principle"
scope: "global"
---

# Symmetry (Express the Same Idea the Same Way)

## Rule

The same idea is expressed the same way everywhere it appears. When two pieces of code are **almost** the
same, split them so the parts that are identical are **literally identical** and the parts that differ are
the **only** visible difference. Symmetry is about form: it does not ask you to merge the two, it asks you to
make the difference legible.

1. **One idea, one shape.** Two functions that answer the same question take their parameters in the same
   order, return the same shape, and name their steps the same way. Ask "what does this do?" of each; the same
   answer from two different shapes is a violation. Which name to pick is settled by the `rust-fn` rule
   document; this document owns the case where two names, orders, or return shapes disagree with each other.
2. **Near-duplicates keep an identical skeleton.** Same step order, same local names, same error handling,
   with the divergence isolated to the lines that must diverge. Reordering steps or renaming locals between
   two variants hides the real difference in noise.
3. **One level of abstraction per function.** Every statement in a body sits at the same altitude. A sequence
   that names intentions does not contain one statement that leaks the mechanism.
4. **Sibling branches carry comparable weight.** Match arms and `if`/`else` branches of one construct all
   delegate, or all inline. A three-line arm beside a thirty-line arm is a missing extraction, visible before
   the logic is read.
5. **Paired operations stay paired.** What is acquired is released, what is encoded is decoded, what is
   registered is deregistered — in the same module, at the same level, in the same vocabulary. The naming
   contract for the inverse is owned by `principle-least-surprise`; what this document requires is that the
   pair exists and sits together.
6. **Sibling modules in the same role share a layout.** Crates or modules that play the same part expose the
   same entry points in the same file positions, so knowing one is knowing all of them.

**Symmetry is not deduplication.** Two symmetric copies with one visible difference are a good outcome, and
often a better one than a single parameterized abstraction; whether to extract at all is decided by
`principle-dry-wet`. Make the pair symmetric first, then decide.

## Examples

1. **One idea, one shape**
   Three lookups that answer the same question about a RomFS image, written three ways.

```rust
// ❌ Bad — same question, three shapes: the image moves between first and last
// parameter, one swallows the error into `None`, and "missing" is spelled as a
// `None`, as an error variant, and as a nested `Option`. Every caller has to open
// the callee to learn which. A helper written against one of them treats "not
// present" as a failure for the second and as success for the third.
pub fn entry(romfs: &RomFs, path: &EntryPath) -> Result<Entry, FromBytesError>;
pub fn get_file_meta(path: &EntryPath, romfs: &RomFs) -> Option<FileMeta>;
pub fn fetch_dir_head(romfs: &RomFs, path: &EntryPath) -> Result<Option<DirId>, FromBytesError>;
```

```rust
// ✅ Good — one shape for one idea: image first, path second, absence is `None`
// and a malformed table is an error. A caller who has used one has used all three,
// and a helper written over one composes with the others unchanged.
pub fn entry(romfs: &RomFs, path: &EntryPath) -> Result<Option<Entry>, FromBytesError>;
pub fn file_meta(romfs: &RomFs, path: &EntryPath) -> Result<Option<FileMeta>, FromBytesError>;
pub fn dir_head(romfs: &RomFs, path: &EntryPath) -> Result<Option<DirId>, FromBytesError>;
```

2. **Near-duplicates keep an identical skeleton**
   Two packers, one per container format. They stay two functions; what changes is that their difference
   becomes visible.

```rust
// ❌ Bad — the same five steps in a different order under different local names, so
// the two real differences (how sections are produced, and what the finish records)
// are buried. A fix to the extract/open ordering that lands in the first is easy to
// miss in the second, because the pair cannot be read side by side.
pub fn pack_nro(elf: &Elf, build_id: BuildId) -> Result<Vec<u8>, PackError> {
    let segments = extract_segments(elf)?;
    let sections = to_sections(&segments);
    let mut builder = NroBuilder::new(build_id);
    for section in sections {
        builder.push(section)?;
    }
    builder.finish()
}

pub fn build_nso_image(elf: &Elf, build_id: BuildId) -> Result<Vec<u8>, PackError> {
    let mut out = NsoBuilder::new(build_id);
    let extracted = extract_segments(elf)?;
    let packed = compress_sections(&extracted);
    out.push_all(packed.sections)?;
    out.finish_with(packed.hashes)
}
```

```rust
// ✅ Good — identical skeleton, identical local names, identical order. Exactly two
// lines differ, and they are the two that must: how sections are produced, and what
// the finish records. A reviewer diffs the pair at a glance and a change to one is an
// obvious prompt to look at the other.
pub fn pack_nro(elf: &Elf, build_id: BuildId) -> Result<Vec<u8>, PackError> {
    let segments = extract_segments(elf)?;
    let mut builder = NroBuilder::new(build_id);
    let sections = to_sections(&segments);
    for section in sections {
        builder.push(section)?;
    }
    builder.finish()
}

pub fn pack_nso(elf: &Elf, build_id: BuildId) -> Result<Vec<u8>, PackError> {
    let segments = extract_segments(elf)?;
    let mut builder = NsoBuilder::new(build_id);
    let sections = to_compressed_sections(&segments);
    for section in sections {
        builder.push(section)?;
    }
    builder.finish_with_hashes()
}
```

3. **One altitude, comparable branches**
   A dispatch over transfer events, where two arms name an intention and the third performs the mechanism.

```rust
// ❌ Bad — the reader has to change altitude mid-match: two arms state what happens,
// the third states how. Reading the ack and resetting the counter are invisible from
// the call site, so a second writer of this match copies the short arms and leaves the
// ack unread, and the next transfer parses it as the head of a chunk.
async fn apply(&mut self, event: TransferEvent) -> Result<(), SendNroError> {
    match event {
        TransferEvent::Started(name) => self.mark_started(name).await?,
        TransferEvent::Chunk(len) => self.mark_sent(len).await?,
        TransferEvent::Finished(summary) => {
            let ack = self.read_ack().await?;
            if ack != ACK_OK {
                return Err(SendNroError::Rejected { ack });
            }
            self.sent = 0;
            ui::status(format!("sent {} ({} bytes)", summary.name, summary.bytes));
        }
    }
    Ok(())
}
```

```rust
// ✅ Good — every arm states an intention and nothing else, so the match reads as a
// list of outcomes. The ack handling lives with the other protocol steps, where the
// next one written will find it.
async fn apply(&mut self, event: TransferEvent) -> Result<(), SendNroError> {
    match event {
        TransferEvent::Started(name) => self.mark_started(name).await?,
        TransferEvent::Chunk(len) => self.mark_sent(len).await?,
        TransferEvent::Finished(summary) => self.mark_finished(summary).await?,
    }
    Ok(())
}
```

## Why It Matters

Asymmetry is paid for on every read. A reader who has understood one member of a pair should be able to skip
the other; when the pair disagrees in shape, they must read both in full and then diff them by hand to find
out whether the difference is meaningful. That cost is invisible in a diff and unbounded over a file's life.

It is paid for again in bugs. Divergent shapes defeat the reviewer's strongest tool, which is noticing that
two things that should match do not: a fix applied to one variant and missed in the other passes review
precisely because nothing looks out of place. Mixed altitude within one body hides effects from the call site,
and unbalanced branches hide a whole procedure inside what reads as a case label.

Symmetry also compounds. Once every format module exposes its parse and its build entry points in the same
positions, and every newtype declares its conversions in the same order, a reader lands in an unfamiliar
module already knowing where to look. That is the return on consistency, and it is only available if the
consistency holds everywhere.

## Pragmatism Caveat

**False symmetry is worse than asymmetry.** Two things that are genuinely different must not be bent into one
shape, because a matching shape is a claim that the behavior matches, and a reader will act on it. Do not pad
a branch to balance it, do not give an infallible function a `Result` so it lines up with its neighbor, and do
not invent a `close()` for a type that owns nothing.

Symmetry is also bounded by the seams around it. A trait from a dependency dictates its own parameter order
and return shape: match the foreign shape at the boundary and the workspace shape everywhere else. Where two
variants are diverging permanently, breaking their symmetry on purpose is the right call, and the cheap
version of it is renaming so the reader stops expecting a pair.

When you break symmetry deliberately, say so in a comment at the declaration. An undocumented asymmetry is
always wrong: the next reader cannot tell it from the copy nobody got around to updating.

## Checklist

Before committing code, verify:

- [ ] Functions answering the same question take the same parameter order and return the same shape
- [ ] Near-duplicate bodies share step order, local names, and error handling; only the intended lines differ
- [ ] No function body mixes statements that name an intention with statements that perform the mechanism
- [ ] Sibling match arms and branches all delegate or all inline; none hides a procedure
- [ ] Every acquire, encode, or register has its inverse in the same module at the same level
- [ ] Modules playing the same role expose the same entry points in the same positions
- [ ] No shape was matched that the behavior does not match, and no branch was padded to balance it
- [ ] Any deliberate asymmetry carries a comment saying why

## References

- [principle-dry-wet](principle-dry-wet.md) - Related: Symmetry makes the difference visible; DRY/WET decides
  whether the pair is one fact and should be extracted at all
- [principle-least-surprise](principle-least-surprise.md) - Related: Owns the naming contract for paired
  operations; a symmetric shape is what makes a name's prediction hold
- [principle-single-responsibility](principle-single-responsibility.md) - Related: A body that mixes altitudes
  is usually a function with two responsibilities
- [principle-open-closed](principle-open-closed.md) - Related: A registry stays extensible only while every
  entry has the same shape
- [principle-rate-of-change](principle-rate-of-change.md) - Related: Says when a symmetric pair should be
  broken on purpose, because the two halves have started moving on different schedules

## External References

- [Symmetry, in Kent Beck's Implementation Patterns](https://blog.iterate.no/2012/06/20/programming-like-kent-beck/)
- [Mastering Programming — Kent Beck](https://tidyfirst.substack.com/p/mastering-programming)
- [The Value of Symmetry — Scott Allen](https://odetocode.com/blogs/scott/archive/2011/02/07/the-value-of-symmetry.aspx)
- [Consistency creates cognitive leverage — A Philosophy of Software Design](https://danlebrero.com/2021/02/24/philosophy-of-software-design-summary/)
- [Single Level of Abstraction Principle](https://principles-wiki.net/principles:single_level_of_abstraction)
