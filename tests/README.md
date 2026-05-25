# droidsaw-dex — tests

Three test binaries plus a fixture tree:

- `tests/adversarial.rs` — triage / OOM / parser-fuzz regression corpus (17 tests). Pure-in-process; no external tools required.
- `tests/dex_roundtrip.rs` — the original `javac → d8 → decompile → javac → java` roundtrip over `tests/fixtures/java/*.java` (20 fixtures, all pass). Hard-coded stdout expectations; does not use the fixture harness.
- `tests/fixture_ratchet.rs` — **tier-1 fixture-harness consumer**. Drives `tests/fixtures/manifest.toml` through `droidsaw_fixture_harness::run_fixture` + `check_ratchet`. Baseline 10 fixtures under `tests/fixtures/java/<Name>/` covering the language-surface categories listed below.

## Fixture layout (`tests/fixtures/`)

```
manifest.toml                  # one [[fixture]] entry per language-surface category
java/
  <Name>/
    src.java                   # source (single public class `<Name>`)
    expected.txt               # golden stdout of running the compiled program
    README.md                  # what this fixture covers + any decompiler quirks
```

Each entry in `manifest.toml` names a fixture, points at `source` + `expected_stdout` (paths relative to `tests/fixtures/`), and carries a `status` of `compile_pass` or `compile_fail` (the only two manifest-storable variants — `semantic_fail` / `resource_limit_exceeded` are outcome-only and rejected at load).

The ratchet contract (`droidsaw_fixture_harness::check_ratchet`):

- `OutcomeKind::SemanticFail` → **always** a regression. There is no "accepted semantic fail" state; fixtures whose stdout genuinely diverges from the baseline have no business in the manifest.
- `OutcomeKind::ResourceLimitExceeded` → **always** a regression.
- `OutcomeKind::CompilePass` when manifest says `compile_fail` → **improvement** (fails the gate intentionally; flip the manifest entry to `compile_pass` to lock the improvement in).
- `OutcomeKind::CompileFail` when manifest says `compile_pass` → regression.
- Missing or unknown fixtures → regression.
- Everything else matches manifest = unchanged.

## Baseline fixture matrix (10)

| name | status | category |
|---|---|---|
| `EmptyClass` | compile_pass | class with no explicit members |
| `SingleMethod` | compile_pass | single static helper + main |
| `IfElse` | compile_pass | if / else-if / else chain |
| `LoopBasic` | compile_pass | for + while |
| `Switch` | compile_pass | dense integer switch with default |
| `TryCatch` | **compile_fail** | try/catch + try/catch/finally |
| `TryResources` | **compile_fail** | try-with-resources over a custom AutoCloseable |
| `Interface` | **compile_fail** | interface type + two implementations |
| `AbstractMethod` | compile_pass | abstract class + concrete subclass |
| `GenericMethod` | **compile_fail** | generic static method over `List<T>` |

The four `compile_fail` fixtures surface distinct decompiler-correctness gaps (drops catch-exception binding, doesn't reconstruct try-with-resources sugar, loses covariant widening on array stores, drops `import java.util.List`). Each fixture's `README.md` documents the exact javac error and the decompiler enhancement that would fix it. When that enhancement is implemented, the ratchet test's `Improvement::CompileFailNowPasses` signal fires and the manifest entry graduates to `compile_pass`.

## Toolchain requirements

- **`javac`** — JDK 17+ recommended; the runner calls with `--release 8`.
- **`java`** — the runtime matched to the installed `javac`. Used to execute both the original program and the decompiled-then-recompiled program.
- **`d8`** — Android SDK build-tools. The runner discovers `d8` via (in order):
  1. `$ANDROID_HOME/build-tools/<version>/d8` (highest version wins),
  2. `d8` on `$PATH`.

If any tool is missing, `tests/fixture_ratchet.rs` synthesizes a `skipped_outcome` per manifest entry (carrying `Warning::RunnerUnavailable`); the ratchet's skip short-circuit lifts each into `RatchetResult.skipped` and the gate stays clean. Test stdout reports `K/N clean (M skipped)` so the skip count is visible. CI that wants to enforce the ratchet must provide the toolchain — set `DROIDSAW_FIXTURE_WARN_AS_ERR=1` to promote any harness warning (including `RunnerUnavailable`) into a test failure.

## Adding a new fixture

1. Create `tests/fixtures/java/<NewName>/` with `src.java` (single public class matching the directory name), `expected.txt` (exact stdout of running `java <NewName>`), and `README.md`.
2. Add a `[[fixture]]` block to `manifest.toml` with the right initial `status`:
   - `compile_pass` if you've confirmed the full roundtrip succeeds on your machine, or
   - `compile_fail` if the decompiled output doesn't recompile / doesn't match — and record the specific gap in the fixture's README.
3. Run `cargo test -p droidsaw-dex --test fixture_ratchet -- --nocapture` and confirm `is_clean()` holds.

`SemanticFail` is never a valid starting status. If a fixture's roundtripped stdout genuinely differs from the original, that's a decompiler-correctness bug surfacing — file it as a new stream, not a manifest entry.

## Running the tests

```bash
# Everything (needs $ANDROID_HOME for dex_roundtrip.rs + fixture_ratchet.rs)
ANDROID_HOME=/path/to/android-sdk cargo test -p droidsaw-dex

# Just the ratchet, with per-fixture trace output
ANDROID_HOME=/path/to/android-sdk \
  cargo test -p droidsaw-dex --test fixture_ratchet -- --nocapture

# Pre-cache the `tests/fixtures/classes.dex` used by lib unit tests
ANDROID_HOME=/path/to/android-sdk bash tests/fixtures/build_fixture.sh
```
