# nx-object

Zero-copy parsing and generation of Nintendo Switch file formats.

The crate is `no_std` by default and opts into `std` through the `filesystem-support` feature, so the
same format definitions serve both host-side tooling and code running on the console.

## Layers

The crate is organized in three layers, each usable on its own:

- **`raw`** -- `#[repr(C)]` binary structure definitions backed by [`zerocopy`]. Direct field access,
  no parsing overhead, always available.
- **`read`** -- Parsing wrappers over the raw structures. Validate magic numbers and sizes, and report
  a typed error per format. Always available.
- **`write`** -- Builders that assemble a format and return the finished image as a byte buffer, so the
  caller chooses where the artifact lands. Requires `filesystem-support`.

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

## Features

| Feature              | Description                                                                |
|----------------------|----------------------------------------------------------------------------|
| `filesystem-support` | Enables the `write` layer and `std`. Required by the other features.        |
| `elf-parsing`        | Derives NRO and NSO segments from a linked ELF binary (`elf` module).       |
| `lz4-compression`    | LZ4 compression and decompression of NSO segments.                          |

No features are enabled by default; that configuration is `no_std`.

## Usage

```toml
[dependencies]
nx-object = { git = "https://github.com/nx-std/nx-object" }
```

## Development

```bash
just check          # cargo check --all-targets
just check-no-std   # cargo check --no-default-features
just test           # cargo nextest run (falls back to cargo test)
just fmt            # cargo +nightly fmt --all
```

## References

- [switchbrew NRO](https://switchbrew.org/wiki/NRO)
- [switchbrew NSO](https://switchbrew.org/wiki/NSO)
- [switchbrew KIP](https://switchbrew.org/wiki/KIP)
- [switchbrew NACP](https://switchbrew.org/wiki/NACP)
- [switchbrew NPDM](https://switchbrew.org/wiki/NPDM)
- [switchbrew RomFS](https://switchbrew.org/wiki/RomFS)

## License

MIT. See [LICENSE](LICENSE).

[`zerocopy`]: https://docs.rs/zerocopy
