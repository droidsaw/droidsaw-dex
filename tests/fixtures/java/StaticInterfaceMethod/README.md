# StaticInterfaceMethod

**Covers.** `static` method on a nested interface (a Java 8 surface).
d8 rejects this construct at `--min-api < 24` ("Static interface
methods are only supported starting with Android N"), so the manifest
pins `d8_min_api = 24` for native support without desugaring.

**Status.** `compile_pass` — full round-trip clean.

**R8 source.** Two-file inline:
- `src/test/examplesAndroidN/interfacemethods/StaticInterfaceMethods.java`
- `src/test/examplesAndroidN/interfacemethods/I1.java`

(Apache-2). Adapted by hoisting `interface I1` to a nested type inside
the test class, so the fixture is self-contained. Class renamed
`StaticInterfaceMethods` → `StaticInterfaceMethod` (singular) to match
the matrix's no-plural convention.

**Outcome.** With `d8_min_api = 24`, d8 leaves `I1.s1()` as a native
static interface method (no desugar to companion class). Decompiler
preserves `static void s1()` on the interface declaration. javac re-
accepts; runtime prints `s1`.

**Notes.** First matrix fixture to exercise `static` on an interface
method. A separate `dex-fixture-harness-expand-via-r8-batch2` (or
similar) could revisit this corner with `d8_min_api < 24` once the
fixture-harness's d8 invocation passes the desugar flag — that path
exercises the companion-class lift, which is a distinct decompile
corner from native preservation.
