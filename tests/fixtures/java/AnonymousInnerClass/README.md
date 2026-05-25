# AnonymousInnerClass

**Covers.** A Java anonymous inner class: `new Runnable() { ... }`. javac compiles this as a named synthetic class `AnonymousInnerClass$1` with a synthetic constructor that captures the enclosing method's effectively-final local (`msg` → synthetic constructor argument + field).

**Status.** `compile_pass` (graduated by the `dex-decompile-nested-classes` stream).

**Fix.** The earlier `compile_fail` was in the test runner, not the decompiler. `tests/fixture_ratchet.rs::decompile` filtered `cls.descriptor.contains('$')` and so never included the synthetic `AnonymousInnerClass$1` class in the recompile set. Once the filter was removed, each class (including synthetics) decompiles to its own file; `new AnonymousInnerClass$1(msg)` at the call site resolves the constructor normally.

The current output does NOT restore the `new Runnable() { ... }` inline form — the synthetic class is emitted as a separate `class AnonymousInnerClass$1 implements Runnable { ... }` file. This compiles + runs correctly and matches the expected stdout, but is structurally less source-faithful than the javac-authored form. Inline restoration (collapse `new AnonymousInnerClass$1()` back to `new Runnable() { ... }`) is a future polish stream — `dex-decompile-anonymous-inline` — not required for fixture graduation.
