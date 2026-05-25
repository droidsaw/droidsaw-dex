# StaticNestedClass

**Covers.** A `static` nested class inside a top-level class. Dalvik emits the nested class as a separate `class_def` whose descriptor is `LOuter$Inner;`.

**Status.** `compile_pass` (graduated by the `dex-decompile-nested-classes` stream).

**Fix.** The earlier `compile_fail` was in the test runner, not the decompiler. `tests/fixture_ratchet.rs::decompile` filtered `cls.descriptor.contains('$')` to skip nested classes, so the inner `StaticNestedClass$Counter` was never decompiled into the recompile set — references from the outer class hit "cannot find symbol". The decompiler's `decompile_class` already handled nested classes correctly; each one produces valid standalone Java output (a top-level class named with `$` characters, which are legal Java identifier characters).

Fix: removed the `$` filter. Each nested class now decompiles to its own `<name>.java` file (with `$` in the filename) inside the recompile src dir, and javac accepts both the file names and the inter-class references.

**Out of scope.** Structural re-nesting (emitting `static class Counter { ... }` INSIDE `class StaticNestedClass { ... }` rather than as a sibling top-level `class StaticNestedClass$Counter { ... }`) is a cosmetic upgrade. The current output compiles and runs correctly; source-level nesting restoration is a future polish stream.
