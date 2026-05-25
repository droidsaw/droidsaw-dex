# R8 library-helper-rename fixture (FP-family empirical anchor)

In-tree counter-example to the `block_outlining` fixture next door.
That fixture shows R8's outliner extracting an identical
`StringBuilder` body into a synthetic helper class — the recogniser
FIRES, the mapping AGREES (`com.android.tools.r8.outline` annotation
present), and the system records a true positive.

This fixture demonstrates the inverse: **R8 minifies library helper
code into short class/method names, the structural recogniser FIRES
on the renamed helpers, but the mapping carries NO outline annotation
on them**. The recogniser's match is a false positive — the helper
class is not an outline, it is just R8-renamed program input that
happened to satisfy the structural predicates I4–I13.

That distinction is the empirical anchor for treating `io.flutter`
helper classes as a source of false positives in outline detection.

## Why this matches flutter_embedding's setup

The Flutter source tree was surveyed. Two findings drive this fixture's
shape:

1. **Flutter's keep rules are minimal.**
   `packages/flutter_tools/gradle/flutter_proguard_rules.pro` is a
   `-dontwarn` + `-keep,allowshrinking,allowobfuscation`
   configuration over `io.flutter.**`. Crucially, it does NOT carry
   a surface-preserving keep — R8 is free to rename every class and
   method in the `io.flutter` namespace to short names. The fixture
   replicates this exactly via the rule
   `-keep,allowshrinking,allowobfuscation class
   fox.droidsaw.r8fixture.libstub.MyLibraryHelpers { *; }`.

2. **flutter_embedding is program input, not library input.**
   `engine/src/flutter/shell/platform/android/BUILD.gn:213-215` and
   `:858-915` show that flutter_embedding is built as a pre-built
   JAR distributed via Maven and consumed by app builds as a regular
   `api` dependency. R8 sees it as program input (subject to
   minification + outlining), not as `--lib` input (which would be
   preserved). The fixture's Kotlin source is similarly compiled to
   a JAR and passed to R8 as `--classfile`.

Net: a real-world Flutter app produces a DEX where R8 has renamed
`io.flutter.plugin.platform.PlatformPlugin` and its 20+ short helper
methods to single-letter classes + methods. The structural
recogniser walks those classes, sees static methods at I4–I13
shape, and fires. The mapping shows the renames, but no
`com.android.tools.r8.outline` annotation, because R8's outliner did
not actually extract anything — the helper methods do subtly
different work and fail R8's identical-body gate.

## The Kotlin source shape (what makes the FP fire)

`source/Main.kt` defines `MyLibraryHelpers` with 24 static methods:

- 12 take an `Int` and produce a tagged-string variant
  (`fmt`, `pad`, `tag`, `hex`, `dec`, `oct`, `bin`, `neg`, `pos`,
  `bra`, `cur`, `pct`).
- 12 take a `String` and produce a formatted variant
  (`upper`, `lower`, `rev`, `pre`, `post`, `wrap`, `trim`, `lenOf`,
  `head`, `tail`, `double`, `bang`).

Each method is straight-line (no branches; satisfies I6), has arity
≤ 2 (I8), body 3–99 bytes (I7), and is invoked from at least two
distinct call sites in `Main.main` (I9 — many callers in the wider
fan-in sense; the recogniser's >= 20 distinct callers gate is
satisfied across the 24-helper × 2-call-site grid).

The bodies are SIMILAR — same general "format a value into a tagged
string" shape — but NOT IDENTICAL. The variation defeats R8's
outliner extract-shared-instruction-sequence pass while leaving the
minifier free to rename every class + method to short names. That's
the load-bearing structural distinction.

## Layout

```
source/
  Main.kt                # 24-helper container + dense-fan-in Main
  proguard-rules.pro     # Flutter-style allowshrinking,allowobfuscation
artifacts/
  .gitkeep               # placeholder; real files land after regen
  classes.dex            # generated
  mapping.txt            # generated
  METADATA.toml          # generated
scripts/
  regen-library-helper-fixture.sh   # kotlinc + R8 pipeline
```

## Regenerating the artifacts

Requires:

- JDK 11+ with `javac` on PATH (or Kotlin's `kotlinc`)
- Android SDK build-tools 34+ providing `d8` and `r8` on PATH, OR
  `R8_JAR` env var pinned to a known R8 release jar
- Optional: `ANDROID_JAR` or `ANDROID_HOME` for the `--lib` target

```sh
./tests/fixtures/r8/library_helper_rename/scripts/regen-library-helper-fixture.sh
```

The script writes `artifacts/classes.dex`, `artifacts/mapping.txt`,
and `artifacts/METADATA.toml`. Re-run after editing the Kotlin
source or bumping R8.

## What the ratchet test asserts

`tests/r8_library_helper_rename_fixture_ratchet.rs`:

1. **SKIP cleanly** if `artifacts/classes.dex` and
   `artifacts/mapping.txt` are missing. Artifacts must be generated
   with the Android SDK and kotlinc, then committed.
2. **Mapping renames present.** The mapping must show
   `MyLibraryHelpers` (or its individual `@JvmStatic` methods) being
   renamed to short names. This confirms the minifier ran.
3. **No outline annotation on the helper class.** The mapping must
   NOT contain `com.android.tools.r8.outline` annotations on the
   renamed `MyLibraryHelpers` slice (kotlin-stdlib's transitive
   classes may produce a few global outline annotations elsewhere —
   that's fine; the assertion is helper-class-scoped).
4. **Recogniser fires.** Walking the renamed DEX, the structural
   recogniser must produce at least one BlockOutline marker on the
   minified helper class.
5. **Markers are false positives.** For every marker fired, looking
   up the (class, method) tuple in the `OutlineSet` must return
   `None` (not outlined per the mapping). The whole marker set on
   `MyLibraryHelpers` is the **DOCUMENTED FP FAMILY** this fixture
   captures empirically.

The fifth assertion is the load-bearing one. It's what justifies
adding `io.flutter` (and any similar library namespace whose keep
rules permit `allowshrinking,allowobfuscation`) to the
known-FP-family allowlist: the marker is structurally well-formed,
the mapping has the final say, and on this engineered fixture the
two disagree by construction.

## Relationship to the BlockOutline gauge

`block_outlining/` proves: when R8 outlines, the recogniser fires
on the right shape. (TP signal — the recogniser works.)

`library_helper_rename/` proves: when R8 only minifies, the
recogniser ALSO fires on the renamed shape. (FP signal — the
recogniser cannot distinguish "synthesized outline" from
"R8-renamed library helper" from DEX shape alone. The mapping is
the disambiguator.)

Together the two fixtures pin both directions of the
`is-this-an-outline?` decision and make the known-FP-family
argument empirical rather than rhetorical.
