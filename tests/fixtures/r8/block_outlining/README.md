# Custom R8 BlockOutline fixture

In-tree ground truth for the R8 BlockOutline recogniser. The Kotlin source under `source/` is engineered to make R8's outliner fire — at least 20 distinct caller methods each invoke an identical 4-invocation `StringBuilder` sequence with a single `Int` param, satisfying R8 source invariants I4–I13 from `r8_inversion::recognise_outline_helper_v2`. Building with R8 9.0+ in `--release` mode + `--pg-conf source/proguard-rules.pro` produces:

- `artifacts/classes.dex` — the optimised DEX
- `artifacts/mapping.txt` — R8's mapping output including one or more `com.android.tools.r8.outline` annotations on the synthetic helper(s)
- `artifacts/METADATA.toml` — R8 version, build timestamp, expected outline count

The artifacts are checked in. Tests under `droidsaw-dex/tests/r8_custom_fixture_ratchet.rs` consume them and assert exact (class, method, kind) tuples against the recogniser. Ground truth is in tree, so a recogniser regression fails the test loudly — unlike the analyst-local mapping-paired ratchets which assert against a corpus that lives outside the repo.

## Why a custom fixture

The precision of outline annotation detection is limited by available corpus data. The F-Droid ecosystem definitively does not publish R8 mappings, and mainstream open-source apps publishing mappings on GitHub releases are limited in coverage (typically ≤ 45 annotations per app). The fastest path to validating against a large set of outline annotations is engineering a synthetic Kotlin source we own that R8 will outline at high density, then checking the artifacts in.

A custom fixture also gives **exact** ground truth — every outlined method's mapping-key class + method + SyntheticKind is known by construction. Per-kind recogniser tests get exhaustive coverage instead of "whatever the corpus happens to ship."

## Regenerating the artifacts

Requires:

- JDK 11+ with `javac` on PATH (or Kotlin's `kotlinc` if the source moves to Kotlin-only)
- Android SDK build-tools 34+ providing `d8` and `r8` on PATH
- Optional: pin the `r8.jar` path via `R8_JAR` env var to a known R8 release

```sh
./tests/fixtures/r8/block_outlining/scripts/regen-r8-fixture.sh
```

The script writes `artifacts/classes.dex`, `artifacts/mapping.txt`, and `artifacts/METADATA.toml`. Re-run after changing the Kotlin source or bumping R8.

## Layout

```
source/
  Main.kt                # the engineered repeated-pattern callers
  proguard-rules.pro     # keep rules + outlining knobs
artifacts/
  .gitkeep               # placeholder; real files land after regen
  classes.dex            # generated
  mapping.txt            # generated
  METADATA.toml          # generated
scripts/
  regen-r8-fixture.sh    # the regen pipeline
```

## Expected outline shape

R8's outliner threshold is 20 distinct callers (`OutlineOptions.threshold = 20`). The fixture provides 30 caller methods invoking the same 4-instruction `StringBuilder` sequence to ensure the threshold is comfortably exceeded. The outlined helper should appear at a class name matching `*$$ExternalSyntheticOutline$*` per `SyntheticNaming.java` — verifiable by greping the mapping for `com.android.tools.r8.outline`.

If a future R8 version raises the threshold or changes the outline-eligibility predicate (I6), the regen will silently produce ZERO outline annotations. The ratchet test fails on `outline_count == 0`, surfacing the drift.
