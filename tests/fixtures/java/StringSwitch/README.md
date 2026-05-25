# StringSwitch

**Covers.** Java 7+ `switch (String)`. javac lowers this to a two-level dispatch: an outer `switch` on `s.hashCode()` whose cases `equals`-check the actual string and assign a synthetic int tag; a second `switch` on the tag runs the user-written case bodies. The Dalvik bytecode contains two packed/sparse switch tables and an `invoke-virtual` to `String.equals` between them.

**Status.** `compile_pass` (graduated 2026-04-23 by `dex-decompile-string-switch`).

**Decompiler gap.** The decompiler leaves the raw two-level form intact: the outer switch is emitted over a `String` discriminant (correct) but with `int` case labels (from the hash-switch cases), which javac rejects:

```
StringSwitch.java:54: error: incompatible types: int cannot be converted to String
    case 2:
         ^
StringSwitch.java:56: error: incompatible types: int cannot be converted to String
    case 1:
         ^
StringSwitch.java:58: error: incompatible types: int cannot be converted to String
    case 0:
         ^
3 errors
```

The true shape is: outer `switch(s.hashCode())` (String discriminant after a `.hashCode()` call) with `case <hash>:` labels (int) — so the discriminant AND the labels are consistent at the hash-switch layer. The decompiler appears to have stripped the `.hashCode()` call from the discriminant (leaving a bare `s`) while keeping the int labels — producing an internally-inconsistent switch. Fixing requires recognising the whole two-switch-plus-equals-chain pattern and collapsing it into a single `switch(s)` over string labels, OR (weaker fallback) restoring the `.hashCode()` call on the discriminant.

**Fix.** Sugar pass `reconstruct_string_switches` in `src/sugar.rs` pattern-matches two adjacent `Stmt::Switch`es where the first switches on a `.hashCode()` result, each outer case body has the canonical `ConstString → equals() → If{..., tag=N}` shape, and the second switches on the tag register. Collects a `tag → literal` map from the outer cases, rewrites the pair as a single new `Stmt::StringSwitch` variant with string case labels. Fail-closed gates: hash-collision outer cases, non-empty outer default, non-canonical case body shape, tag-label mismatch between outer + inner, tag-var liveness in inner default. The dead `int v0 = s.hashCode();` local remains in the output (Java accepts unused locals; DCE is a future cleanup).
