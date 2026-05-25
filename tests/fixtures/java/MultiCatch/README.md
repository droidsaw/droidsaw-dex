# MultiCatch

**Covers.** `catch (A | B | C e)` multi-catch — one handler, multiple exception types, single binding. In Dalvik this compiles to multiple `exception_handler` entries that all point at the same `handler_off`; the handler body begins with a single `move-exception` into the shared catch register.

**Status.** `compile_pass` (graduated by the `dex-decompile-multicatch` stream).

**Fix.** The earlier `compile_fail` had two related defects: (a) N sibling catch clauses with duplicated bodies (one per type); (b) the body referenced a phi-merged VarId (`v1_14`) that wasn't bound in any individual catch — SSA had phi-merged the three catch-bound vars (`v1_11`, `v1_12`, `v1_13`) into a unified `v1_14` at the shared handler-body entry, and our emit rendered `v1_14.getMessage()` without binding it.

The emit-layer collapse pass in `src/emit.rs::Stmt::TryCatch` now groups consecutive CatchClauses whose body Stmt trees are byte-identical (via Debug string comparison) and emits them as a single clause with union types and a single binding. For the binding name, the new helper `catch_binding_from_body_phi` walks the body's uses looking for the first VarId that references the catch-register — the phi-merge result — and uses that as the merged catch binding. This makes the body's pre-existing references resolve transparently without requiring a body rewrite:

```java
catch (IllegalStateException | NumberFormatException | UnsupportedOperationException v1_14) {
    return v1_14.getMessage();  // v1_14 is now the catch binding; body unchanged
}
```

**Safety guards.**
- Only consecutive catches with byte-identical body Stmt trees collapse — heterogeneous catches in the same TryCatch (different bodies) emit separately.
- Single-catch (non-collapsed) path unchanged — uses the original `c.var` or falls back through the existing hoisted-var / "e" cases.
- Body-equality via `format!("{:?}", &body)` Debug strings is sufficient for the current structure pass's output (all three MultiCatch bodies produce identical Debug strings modulo the catch binding which lives on the `CatchClause` struct, not in the `body` field).

**Out of scope.**
- Mixed-body multi-catch where Dalvik produces multiple handlers sharing SOME pattern but not identical bodies. Not currently observed; future fixture surface.
- Method-signature `throws A, B` reconstruction. Separate concern.
