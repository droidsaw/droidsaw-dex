# R8 AndroidX-style library-helper rename fixture

In-tree fixture demonstrating a recogniser **false positive**: library-helper methods in an AndroidX-shape package (`androidx.testlib.*`) get minified by R8 to short identifiers (`a`, `b`, `c`, ...) but R8 does NOT outline them. The recogniser's I4-I13 structural predicates fire on the renamed bodies (they happen to satisfy the invariants by coincidence — small, static, straight-line, single-arg) even though the mapping carries zero `com.android.tools.r8.outline` annotations on those tuples.

This fixture is the empirical anchor for a proposed `androidx` entry in the recogniser's family FP-suppression list. It is the AndroidX-named sibling of `tests/fixtures/r8/library_helper_rename/` (Flutter-style, neutral package), demonstrating that the FP pattern is family-shaped: library helpers minified to short names from multiple ecosystems converge on the same structural shape.

## What this fixture demonstrates

R8's outliner and R8's minifier are **different passes**:

- **Outliner** extracts repeated bodies across >= 20 distinct callers into a synthetic helper class. The mapping records this with a `com.android.tools.r8.outline` annotation on the helper. This is what the BlockOutline recogniser is *intended* to detect.
- **Minifier** renames every kept class and method to a short identifier. The mapping records the rename via the `original -> obfuscated:` record shape. Library-helper classes (here: `androidx.testlib.LifecycleHelpers`) get renamed but their bodies remain in place.

The recogniser fires on the structural shape of a single method body, not on the mapping evidence. When library helpers are small, static, and structurally similar to a R8-synthesised outline body (3-10 instructions, single param, no branches), the recogniser cannot distinguish them from genuine outline helpers without the family FP list.

## Why an AndroidX-prefixed stub package

AndroidX is the most common library family in production Android apps that R8 will minify aggressively when the consumer app uses ProGuard/R8 release builds without `-keep class androidx.** { *; }` rules. Real AndroidX deps host hundreds of small static helper methods in shapes nearly identical to what the recogniser was designed to flag as outlines. A family entry keyed on `androidx.*` package prefix lets the recogniser suppress this class of FP.

The stub package name `androidx.testlib` avoids any collision with a real AndroidX subnamespace (e.g., `androidx.lifecycle`, `androidx.appcompat`) — `testlib` is unambiguously a synthetic-fixture marker.

## Important distinction from the BlockOutline fixture

The neighbour fixture `tests/fixtures/r8/block_outlining/` is engineered so R8 *will* outline (30 identical callers of a 4-instruction body per group). This fixture is engineered so R8 will *not* outline — each helper body is structurally different enough that the outliner's repeated-body predicate rejects them. The mapping should contain zero `com.android.tools.r8.outline` annotations on `androidx.testlib.*` classes after R8 runs.

## Regenerating the artifacts

Requires:

- JDK 11+ with `javac` on PATH and `kotlinc` (Kotlin 1.9+) installed
- Android SDK build-tools 34+ providing `r8` on PATH, OR `R8_JAR` env var pointing at an R8 release JAR

```sh
./tests/fixtures/r8/androidx_helper_rename/scripts/regen-androidx-helper-fixture.sh
```

Outputs:

- `artifacts/classes.dex` — minified DEX
- `artifacts/mapping.txt` — R8's mapping; expect zero outline annotations on `androidx.testlib.*` but presence of class renames
- `artifacts/METADATA.toml` — R8 version, source SHA-256s, output count

## Ratchet test

`tests/r8_androidx_helper_rename_fixture_ratchet.rs` asserts:

1. SKIP cleanly if `classes.dex` + `mapping.txt` are absent (default state — artifacts are not checked in until a regen is exercised on a machine with the toolchain).
2. Mapping contains at least one class rename inside the `androidx.testlib.*` namespace (evidence R8 minified the helpers).
3. Mapping does NOT contain `com.android.tools.r8.outline` annotations on the renamed helpers' tuples — they are minified library code, not R8-synthesised outlines.
4. Running the recogniser on the DEX fires at least one marker on a renamed `androidx.testlib.*` class.
5. **Key assertion:** every marker that fires is NOT present in the mapping's `OutlineSet`. The markers are unambiguous false positives — the recogniser fires structurally, the mapping disagrees.

## Layout

```
source/
  Main.kt                          # androidx.testlib.LifecycleHelpers + Main entry point
  proguard-rules.pro               # keep Main only; let R8 fully minify helpers
artifacts/
  .gitkeep                         # placeholder; real files land after regen
  classes.dex                      # generated
  mapping.txt                      # generated
  METADATA.toml                    # generated
scripts/
  regen-androidx-helper-fixture.sh # kotlinc -> R8 release pipeline
```
