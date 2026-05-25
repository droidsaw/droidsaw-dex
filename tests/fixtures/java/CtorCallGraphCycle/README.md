# CtorCallGraphCycle

**Covers.** Constructor with conditional branch + super() call where
the inner branch instantiates a peer object that re-enters the chain
at one level shallower. Tests the ordering of `<init>` body statements
relative to super() invocation across an A↔B mutual-instantiation
cycle (terminating because `alwaysFalse` prunes the inner conditional
on the second descent).

**Status.** `compile_fail` — recompile stage fails on two distinct
defects.

**R8 source.** `src/test/examples/classmerging/CallGraphCycleTest.java`
(Apache-2). Adapted by removing R8-only println-formatting tweaks; no
structural changes.

**Outcome.** Two recompile errors:

1. `new CtorCallGraphCycle$B()` is emitted with no args, but B's only
   ctor takes `(boolean, boolean)`. The decompiler dropped the args on
   the inner `new B(...)` site inside `A.<init>`'s conditional branch.
2. `int cannot be converted to boolean` on the outer `main`'s
   `args.length == 0` boolean expression — type inference is leaking
   `int` where `boolean` is expected, suggesting the comparison
   instruction's result type isn't being widened to `boolean`.

**Candidate follow-up.** `dex-decompile-ctor-args-and-bool-types` —
compound stream covering (a) ctor-arg propagation through `<init>`
cycles where the receiver class is the SAME as the constructor's
declaring class (sub-defect on `new B(args)` resolution), and (b)
boolean-vs-int discrimination on the result of `if-eqz` / `if-nez`
sequences that produce a boolean value (currently leaking int).
