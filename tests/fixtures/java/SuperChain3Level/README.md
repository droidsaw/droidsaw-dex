# SuperChain3Level

**Covers.** 3-level `super.m()` invocation chain (`C extends B extends A`,
each overrides `m()` and chains via `super.m()`). DEX `invoke-super`
(opcode `0x6F`) resolves on the static class context, not the dynamic
receiver, so each link in the chain is a distinct invoke-super site
referencing the immediate parent.

**Status.** `compile_fail` — RunRecompiled times out at 20s.

**R8 source.** `src/test/examples/classmerging/RewritePinnedMethodTest.java`
(Apache-2). Adapted by removing the R8 keep-rule comment + simplifying
the entry-point `main` to a direct `new C().m()` call (R8 source had
intermediate variables for keep-rule reasons).

**Outcome.** Recompile succeeds — the decompiler emits syntactically
valid Java — but the runtime hangs. The most likely defect: the
decompiler collapsed `invoke-super` to a plain method-call form (`m()`),
which on the same receiver re-enters the same class's override and
infinite-loops. Recompile is happy because `this.m()` is type-valid;
the failure surfaces only at runtime as unbounded recursion (eventually
StackOverflowError, but the per-process wall-time cap fires first).

**Candidate follow-up.** `dex-decompile-invoke-super-preservation` —
discriminate `invoke-super` (`0x6F` / `0x76`) from `invoke-virtual`
(`0x6E` / `0x74`) at the emit boundary. Currently both appear to lower
to plain `obj.method(args)`; the super variant must lower to
`super.method(args)` when the receiver is `this` and the target class
is the declaring class's immediate super.
