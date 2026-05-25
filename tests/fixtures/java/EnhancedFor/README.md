# EnhancedFor

**Covers.** Enhanced-for loop over an `Iterable<Integer>` (sibling `ForEach.java` under the roundtrip test set already covers arrays). javac compiles the iterable form to `Iterator<T> it = c.iterator(); while (it.hasNext()) { T x = it.next(); ... }`.

**Status.** `compile_fail` at `Recompile` stage.

**Decompiler gap.** `java.util.List` is not imported into the decompiled output, so the method signature references an unresolved type:

```
EnhancedFor.java:39: error: cannot find symbol
    static int sumList(List v2) {
                       ^
  symbol:   class List
  location: class EnhancedFor
1 error
```

Same missing-import root cause as the `GenericMethod` fixture (which drops `import java.util.List`). Both fixtures seed the already-queued `dex-decompile-generics-and-imports` stream. Additionally, whether the `Iterator`-based bytecode is restored back to `for (Integer x : xs)` syntax is masked by the import error — once imports land, the for-each restoration (if missing) will surface as a separate `SEMANTIC_FAIL` (different output) rather than a `COMPILE_FAIL`, driving a follow-up fixture refinement.

**Candidate follow-up streams.** Primary: `dex-decompile-generics-and-imports` (already queued) — once imports are restored this fixture should advance to either `compile_pass` or surface a residual `SEMANTIC_FAIL` signalling the iterator-pattern restoration gap. Secondary: `dex-decompile-enhanced-for-iterable` — pattern-match `iterator()` + `hasNext()` + `next()` loop shape to restore `for (T x : c)`.
