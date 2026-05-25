# TryResources

**Covers.** `try-with-resources` over a custom `AutoCloseable`. javac expands this into a nested `try / finally` with an `addSuppressed` helper and a `Throwable` local holding any primary exception; the decompiler's structuring pass reconstructs the nested exception-region shape, and the emit layer hoists the cross-catch escaped SSA var out to method scope.

**Status.** `compile_pass` (graduated by the `dex-decompile-try-resources` stream).

**Fix.** The earlier failure was a cross-catch SSA-scope leak: the Dalvik `move-exception` in the primary handler binds a register (rendered as `v1_3`) that is then re-used from a sibling catchall handler's `addSuppressed(...)`/`throw`. The inner catch's Java-source scope would not cover the outer catch's uses. `src/emit.rs` now runs a method-level pre-pass (`collect_escaped_catch_vars`) that detects catch-bound SSA vars whose uses escape their legitimate scope (the union of all catch bodies binding the same var in the same TryCatch), hoists each such var to a method-scope `RuntimeException v = null;` declaration, re-binds the catch clause to a fresh tmp (`_c_{reg}_{ver}`), and prologues the body with `v = (RuntimeException) tmp;` so the hoisted local is in scope wherever the SSA var was originally referenced.

The `RuntimeException` narrowing (with a cast on assign) is the pragmatic trade-off: it sidesteps Java's checked-exception rule for `throw v;` without cascading a `throws` declaration through the class's call graph. For this fixture the catch bodies are unreachable at runtime (the `Resource.read()` / `Resource.close()` overrides don't throw), so the cast is dead code. A fixture with an actually-thrown non-`RuntimeException` would `ClassCastException` on entry to the prologue — captured as follow-up `dex-decompile-throws-analysis`.

Sugar reconstruction (collapsing the nested try/catch back into `try (Resource r = new Resource(tag))`) stays out of scope per the stream's brief — that's a Phase 3+ pattern-library task. The current output remains in "lowered" form, just structurally valid.
