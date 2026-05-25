# GenericMethod

**Covers.** A generic static method (`<T> T first(List<T>, T)`) with two call sites over different type arguments. DEX strips generic signatures to erased types (`Object`, `List`); the fixture exercises both the erasure itself and the decompiler's import emission for referenced stdlib types.

**Status.** `compile_pass` (graduated by the `dex-decompile-short-circuit-or-structuring` stream).

**Fix history.** Two streams landed the pieces:

1. **`dex-decompile-generics-and-imports` @ `43818c7`** — fixed the missing-import gap. Before this stream, the decompiled output referenced `List` as a bare identifier without emitting `import java.util.List;`, so javac rejected with `cannot find symbol List`. The class-level FQN accumulator in `src/classes.rs` now prepends a sorted `import X.Y.Z;` block between the package declaration and the class header.

2. **`dex-decompile-short-circuit-or-structuring` @ <this stream>** — fixed the residual short-circuit `||` structuring gap. After the import fix, javac rejected on a DIFFERENT error: "missing return statement". The structuring pass had been splitting the javac-lowered `if (xs == null || xs.isEmpty()) return fallback;` guard into nested if-else (`If{A, Return(X), Some(Seq[setup, If{B, Return(Y), None}])}`) where the inner else falls through — in Dalvik the fall-through reached a shared `:label1 return fallback` that the outer-then had already absorbed, leaving the inner-else without a reachable return.

   The fix in `src/structure.rs::reconstruct_short_circuit_guards` is a post-structure rewrite pass: it detects the exact shape above (at method-body tail only, with strict type-guards on the terminator shape) and rewrites to two sibling guards plus a trailing duplicate `Return(X)`:

   ```
   before:   If{A, Return(X), Some(Seq[setup, If{B, Return(Y), None}])}
   after:    [If{A, Return(X), None}, setup, If{B, Return(Y), None}, Return(X)]
   ```

   Semantically identical to `if (A || negate(B)) return X; return Y;` without requiring a `LogicalOr` Condition variant — two sibling if-guards cover the OR.

**Duplicating `Return(X)` is safe** because the pattern match deliberately restricts the OUTER then_body to `Stmt::Return(Option<VarId>)` (bare VarId or void; no `InlinedReturn`/`InlinedThrow`/`StringConcat` with side effects). The INNER return can be any return-like (it only appears once in the rewrite).

**Out of scope for this fixture (tracked separately).**
- **Generic erasure**: `<T> T first(List<T>, T)` decompiles to `Object first(List, Object)`. DEX-level signature stripping is fundamental, not fixable via source-layer re-synthesis. Tracked as `dex-decompile-generic-signatures-pass` — reads the `dalvik.Signature` annotation (when present) and re-threads generics.
- **Sugar polish**: the decompiled output uses `Integer.valueOf(-1)` where the source had `-1`. Java auto-boxing is erased at bytecode level; explicit `valueOf` is semantically correct. Not worth un-desugaring.

**Output shape**:

```java
static Object first(List v1, Object v2) {
    if (v1 == null) {
        return v2;
    }
    boolean v0 = v1.isEmpty();
    if (!v0) {
        return v1.get(0);
    }
    return v2;
}
```

Compiles cleanly + produces the same runtime output as the original source.
