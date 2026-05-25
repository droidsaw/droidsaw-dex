# PrivateMethodCollision

**Covers.** Two distinct sub-corners.

1. Private `m()` declared on both `A` and `B extends A`. Private
   methods do NOT participate in dynamic dispatch — `A.m` and `B.m`
   are unrelated symbols even though declared in parent/child classes.
   `A.invokeM()` (public) calls `m()` and resolves at compile time to
   `A.m`, even when invoked on a B receiver.
2. `D extends C` with a public `m()` override + `super.m()` chain to
   `C.m`. Standard virtual dispatch + invoke-super.

The discriminator is `invoke-direct` (private, `0x70`) vs
`invoke-virtual` (`0x6E`).

**Status.** `compile_fail` — RunRecompiled times out at 20s.

**R8 source.** `src/test/examples/classmerging/MethodCollisionTest.java`
(Apache-2). Adapted by stripping the `@NeverInline escape` helper,
removing the unused `B.m` invocation site (the original test invoked
`b.m()` directly via reflection-style; for our pipeline we invoke
through `b.invokeM()` to make the corner active without reflection),
and renaming `m` → keep-as-`m` for clarity.

**Outcome.** Recompile succeeds; runtime hangs. Same defect class as
`SuperChain3Level`: the decompiler likely rewrote the `invoke-direct`
in `A.invokeM`'s `m()` call to a virtual `m()` call, which on a B
receiver dispatches to `B.m` — and `B.m` either re-enters via the
same path or produces wrong output that loops. Either way the
per-process timeout fires.

**Candidate follow-up.** `dex-decompile-private-dispatch-preservation`
— preserve `invoke-direct` (`0x70`) on a private method as a
self-class direct call, distinct from `invoke-virtual` (`0x6E`).
Likely sibling to `dex-decompile-invoke-super-preservation` (filed
from `SuperChain3Level`); both surface the broader "method-resolution
opcode collapse" defect family.
