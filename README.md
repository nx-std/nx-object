# nx-object

Zero-copy parsing and generation of Nintendo Switch file formats.

Turns a byte buffer into a validated view of an NRO, NSO, NACP, NPDM, or RomFS image, and builds
those formats -- and the NCA and CNMT a title is distributed as -- back out of their parts. Nothing
is copied to read an image and nothing is written to disk to produce one, so the same definitions
serve a host-side packer and code running on the console.

## Layers

The crate is organized in three layers, each usable on its own:

- **`raw`** -- `#[repr(C)]` binary structure definitions backed by [`zerocopy`]. Direct field access,
  no parsing overhead, no allocator.
- **`read`** -- Parsing wrappers over the raw structures. Validate magic numbers and sizes once, then
  hand out parts without further checks, and report a typed error per format. No allocator.
- **`write`** -- Builders that assemble a format and return the finished image as a byte buffer, so
  the caller chooses where the artifact lands. Needs a heap; the builders that walk a directory need
  a filesystem too.

A fourth module, `elf`, extracts the segments of a linked ELF binary and hands them to the NRO and
NSO builders.

## Formats

| Format | Description                                    | `raw` | `read` | `write` |
|--------|------------------------------------------------|:-----:|:------:|:-------:|
| NRO    | Nintendo Relocatable Object (homebrew)         |   ✓   |   ✓    |    ✓    |
| NSO    | Nintendo Software Object (system module)       |   ✓   |   ✓    |    ✓    |
| KIP    | Kernel Initial Process                         |   ✓   |        |    ✓    |
| NACP   | Nintendo Application Control Property          |   ✓   |   ✓    |    ✓    |
| NPDM   | Nintendo Program Description Metadata          |   ✓   |   ✓    |    ✓    |
| RomFS  | Read-only filesystem image                     |   ✓   |   ✓    |    ✓    |
| PFS0   | Partition filesystem archive                   |   ✓   |        |    ✓    |
| MOD0   | Module header embedded in executables          |   ✓   |   ✓    |         |
| NCA    | Nintendo Content Archive                       |   ✓   |        |    ✓    |
| CNMT   | Content meta naming every NCA of a title       |   ✓   |        |    ✓    |

## What this crate does not do

It does not sign, encrypt, or verify anything: an NPDM's ACID signature is stored and reproduced but
never checked, and an NSO's segment hashes are computed on write yet left to the caller on read. It
also does not decompress, because a reader that borrows its buffer has nowhere to put the expanded
bytes.

NCA is where that line is most visible, because an NCA on disk is encrypted throughout. `NcaBuilder`
produces the plaintext container and every hash covering it, then names what is still owed; the
caller supplies the keyset and the ciphers. Hashing stays here because a hash is part of the layout
-- it is what makes the recorded offsets checkable -- while encryption is a transformation applied to
a layout that is already correct.

## Usage

```toml
[dependencies]
nx-object = { git = "https://github.com/nx-std/nx-object" }
```

The default build carries every format and the standard library. A consumer that wants less -- one
format, or a build with no allocator and no OS -- selects it through the crate's Cargo features,
which are documented on each entry in [`Cargo.toml`](Cargo.toml).

## Development

```bash
just check --all-targets --all-features   # compile check
just clippy --all-targets --all-features  # lint
just test --all-features                  # cargo nextest run (falls back to cargo test)
just check-unused-deps                    # cargo machete
just fmt                                  # cargo +nightly fmt --all

# The bare-metal half, the way CI builds it
just check --no-default-features --features all-formats,alloc --target aarch64-unknown-none
just clippy --no-default-features --features all-formats,alloc --target aarch64-unknown-none
```

## References

Every format the crate covers is documented on the [switchbrew] wiki, except RomFS, whose layout the
console inherits unchanged from the 3DS.

- [NRO](https://switchbrew.org/wiki/NRO)
- [NSO](https://switchbrew.org/wiki/NSO)
- [KIP1](https://switchbrew.org/wiki/KIP1)
- [NACP](https://switchbrew.org/wiki/NACP)
- [NPDM](https://switchbrew.org/wiki/NPDM)
- [PFS0](https://switchbrew.org/wiki/NCA#PFS0)
- [MOD](https://switchbrew.org/wiki/MOD)
- [NCA](https://switchbrew.org/wiki/NCA)
- [CNMT](https://switchbrew.org/wiki/CNMT)
- [RomFS](https://www.3dbrew.org/wiki/RomFS)

## License

MIT. See [LICENSE](LICENSE).

[`zerocopy`]: https://docs.rs/zerocopy
[switchbrew]: https://switchbrew.org/wiki/Main_Page
