# droidsaw-dex fuzz harness

libFuzzer targets for `droidsaw-dex`. Each target exercises a P0
no-panic invariant on a parser / decoder / CFG / SSA path.

## Targets

| Target               | Invariant                                                       |
|----------------------|-----------------------------------------------------------------|
| `fuzz_parser`        | `DexFile::parse(bytes)` never panics on arbitrary input.        |
| `fuzz_opcode_decode` | `decode::decode_insns(bytes, 0, len/2)` never panics.           |
| `fuzz_cfg`           | Parse → walk methods → `Cfg::build(code)` never panics.         |
| `fuzz_ssa`           | Parse → walk methods → `Cfg::build` → `SsaBody::build` no panic.|

`fuzz_opcode_decode` stands in for the "opcode encode/decode
involution" property until a DEX bytecode re-encoder exists.

## Prerequisites

- `cargo-fuzz` (`cargo install cargo-fuzz`).
- Rust nightly toolchain (nightly is required by libFuzzer's sanitizer flags).
  On this machine: `RUSTUP_TOOLCHAIN=nightly-2025-11-21`.

## Run

From `droidsaw-dex/`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
export RUSTUP_TOOLCHAIN=nightly-2025-11-21

cargo fuzz run fuzz_parser         # until you ctrl-c
cargo fuzz run fuzz_opcode_decode  -- -max_total_time=60  # bounded 60s smoke
cargo fuzz run fuzz_cfg            -- -max_total_time=60
cargo fuzz run fuzz_ssa            -- -max_total_time=60
```

Corpus-only sanity check (build + load seeds, no mutation):

```sh
cargo fuzz run <target> -- -runs=0
```

## Dictionaries

libFuzzer dictionaries under `fuzz/dictionaries/` anchor mutation to
the format's known magic bytes, section type codes, and opcode bytes.
Provenance cited in each file's header.

| Target                       | Dictionary                  |
|------------------------------|-----------------------------|
| `fuzz_parser`                | `dictionaries/dex.dict`     |
| `parser_differential`        | `dictionaries/dex.dict`     |
| `fuzz_opcode_decode`         | `dictionaries/dex.dict`     |
| `fuzz_emit_roundtrip`        | `dictionaries/dex.dict`     |
| `fuzz_cfg`                   | `dictionaries/dex.dict`     |
| `cfg_differential`           | `dictionaries/dex.dict`     |
| `fuzz_ssa`                   | `dictionaries/dex.dict`     |
| `fuzz_emulator`              | `dictionaries/dex.dict`     |
| `fuzz_protector_recognizer`  | `dictionaries/dex.dict`     |
| `fuzz_enum_cross_class`      | `dictionaries/dex.dict`     |

Append `-- -dict=fuzz/dictionaries/<name>.dict` to any `cargo fuzz run`:

```sh
cargo fuzz run fuzz_parser -- -dict=fuzz/dictionaries/dex.dict
cargo fuzz run fuzz_parser -- -dict=fuzz/dictionaries/dex.dict -max_total_time=60
```

## Layout

```
fuzz/
├── Cargo.toml
├── fuzz_targets/
│   └── fuzz_*.rs
├── seeds/
│   └── <target>/    — tracked, hand-curated seeds (this is the canonical set)
├── corpus/
│   └── <target>/    — libFuzzer's runtime working corpus (gitignored; machine-local)
└── crashes/
    └── <target>/    — tracked reproducers (short input-hash filenames) + .note files
```

Before a run, seed the working corpus from `seeds/`:

```sh
for t in fuzz_parser fuzz_opcode_decode fuzz_cfg fuzz_ssa; do
    cp -n fuzz/seeds/$t/* fuzz/corpus/$t/ 2>/dev/null || true
done
```

`corpus/*/*` is gitignored except `.gitkeep`. Mutation artefacts stay local.

## Crash triage

If a run produces a crash:

1. `cargo fuzz tmin <target> <input>` to minimize.
2. Move the minimized input to `fuzz/crashes/<target>/<hash>` (8-char input SHA-1
   prefix is fine).
3. Write a companion `fuzz/crashes/<target>/<hash>.note` with:
   - stage that panicked (parser / decode / cfg / ssa)
   - panic message (one line, trimmed)
   - repro: `cargo fuzz run <target> fuzz/crashes/<target>/<hash>`

Fixing the underlying panic belongs to a hardening pass; the harness
itself only catches and records.
