# Custom D8 desugar fixture (j$/* are NOT R8 outlines)

In-tree empirical anchor for the structural claim: classes in the
`Lj$/` namespace are D8 desugared-library backports, NOT R8 outline
helpers. The Kotlin source under `source/` is engineered to invoke
enough `java.time` + `java.util` + `java.util.stream` API surface that
D8 emits desugared-library backport classes under `Lj$/time/`,
`Lj$/util/`, and `Lj$/util/stream/`. Building with D8 (`--min-api 24`,
which forces desugaring of `java.time` and `java.util.stream` on
pre-API-26 targets) followed by R8 in release mode produces:

- `artifacts/classes.dex` — the optimised DEX containing `Lj$/*` backport classes alongside `Main` and friends
- `artifacts/mapping.txt` — R8's mapping output. The `j$.*` class records carry NO `com.android.tools.r8.outline` annotation by construction
- `artifacts/METADATA.toml` — R8 + D8 versions, build timestamp, j$ class count, outline annotation count

The artifacts are NOT generated in this commit — the Mac dev box does
not carry the Android SDK. The regen script will be exercised on the
the development host, which has the r8-9.0.32.jar artifact plus the
desugared-library config. Until then, the ratchet test SKIPs cleanly.

## The structural claim

R8's `BlockOutlinerOptions` filter at `OutlinerImpl.java:1593-1595`
excludes synthetic classes from outline emission, but more
importantly the desugared-library track operates as a separate
*input* to R8 (an L8-compiled prebuilt) rather than as code the
outliner can produce. The pipeline is:

1. D8 desugars `java.time.LocalDate` calls into `j$.time.LocalDate` backport-class calls, emitting the backport classes into the DEX
2. R8 sees `j$.*` as ordinary input classes and may rename or inline
   them, but it does NOT emit new methods into the `j$.*` namespace —
   that namespace is reserved for the L8 / desugared-library tool, and
   R8's `LibraryDesugaredChecker` ensures the boundary is respected

Source citations from R8 source code analysis (version
`9.0.32`-adjacent):

- `LibraryDesugaredChecker.java:81` — the gate that prevents
  R8-generated synthetics from landing in the desugared-library
  namespace.
- `SyntheticNaming.java:104-139` — the synthetic-kind enumeration. No
  variant emits into `j$/`; the outline kinds emit into
  `<context>$$ExternalSyntheticOutline$N` only.
- `OutlinerImpl.java:1593-1595` — the outliner's eligibility check
  excludes class members that come from desugared-library inputs.

This fixture *demonstrates* the claim empirically: a build engineered
to make D8 emit a population of `j$.*` backport classes, then run
through R8, then inspected for the presence of `com.android.tools.r8.outline`
annotations on those classes. The annotations must not be there.

## Why a custom fixture

Source inspection shows that `j$/*` are nominally out-of-scope for R8
outline emission (controlled by `LibraryDesugaredChecker.java`). A
source-only argument is fragile: an R8 version bump could in principle
relax that check and start emitting outlines into the desugared-library
namespace. An in-tree fixture makes this a runtime-verified property —
if R8 ever emits outlines into `j$.*`, this fixture's assertion fires
and surfaces the issue as a test failure.

A custom fixture also gives EXACT ground truth — every `j$.*` class
in the DEX is known by construction to be a D8 backport, not an R8
outline. Consumers of the validation gauge can therefore safely tag
`Lj$/`-namespaced helpers as "not an outline candidate" without
risking a false negative.

## Regenerating the artifacts

Requires:

- JDK 11+ with `java` on PATH
- Kotlin compiler (`kotlinc`) 1.9+ on PATH
- Android SDK build-tools 34+ providing `d8` on PATH, OR pinned via `D8_JAR=/path/to/d8.jar`
- R8 9.0+ via `R8_JAR=/path/to/r8.jar`
- Desugared-library config + dex prebuilt: `DESUGARED_LIB_JSON=/path/to/desugar_jdk_libs_configuration.json` + `DESUGARED_LIB_JAR=/path/to/desugar_jdk_libs.jar`. These ship with R8 9.0.32 at the standard Android SDK locations.

```sh
./tests/fixtures/r8/d8_desugar/scripts/regen-d8-desugar-fixture.sh
```

The script writes `artifacts/classes.dex`, `artifacts/mapping.txt`,
and `artifacts/METADATA.toml`. Re-run after changing the Kotlin
source or bumping R8 / D8 / the desugared-library config.

## Layout

```
source/
  Main.kt                # the engineered java.time + java.util backport callers
  proguard-rules.pro     # keep rules; allow optimization (mostly default R8 release)
artifacts/
  .gitkeep               # placeholder; real files land after regen
  classes.dex            # generated
  mapping.txt            # generated
  METADATA.toml          # generated
scripts/
  regen-d8-desugar-fixture.sh    # the regen pipeline
```

## Min-api choice

The regen script uses `--min-api 24`. At min-api 24:

- `java.time.*` (API 26+) is desugared into `j$/time/*` backports
- `java.util.stream.*` (API 24 is the minimum for some, but conservative ones get backports) is desugared
- `java.util.Optional` (API 24+) does NOT get desugared at min-api 24, since 24 already provides it natively. So Optional helps the source build but does not contribute to the j$ class count

This choice exercises the dominant `j$/time/` and `j$/util/stream/`
families, which is enough for the structural claim. Setting
`--min-api 21` would additionally desugar `java.util.Optional` into
`j$/util/Optional` but is not necessary for the fixture's purpose.
The METADATA.toml records the choice for reproducibility.

## Expected fixture shape

After regen, the mapping should contain:

- One or more class records whose LHS (original-name half) starts
  with `j$.` — these are the D8 desugared-library backport classes
- Zero `com.android.tools.r8.outline` annotations attached to any of
  those `j$.*` class records
- Possibly a small number of `com.android.tools.r8.outline`
  annotations attached to NON-`j$` classes (R8 may outline repeated
  patterns in `Main` or in the backports' use sites). The ratchet's
  positive-control assertion requires at least one non-`j$` outline
  marker to fire — without it, the test isn't actually exercising the
  recogniser

If R8 ever starts emitting outlines into the `j$.*` namespace, the
ratchet's negative assertion will fire and the structural claim's
empirical floor will surface as a loud test failure.
