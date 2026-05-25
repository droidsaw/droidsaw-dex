# OverloadsPeerClasses

**Covers.** Symmetric overloads `A.m(B)` and `B.m(A)` between two
unrelated classes. `ASub extends A` and `BSub extends B`, so the
dynamic types of the receivers (ASub, BSub) are subclasses but the
param types (B, A) are the static base classes.

**Status.** `compile_fail` — recompile stage fails.

**R8 source.** `src/test/examples/classmerging/SyntheticBridgeSignaturesTest.java`
(Apache-2). Adapted by stripping the `@NeverInline escape` helper. The
classes are kept as `private static class` to match the R8 source's
visibility (which is what triggers the synthetic-accessor pattern
under test).

**Outcome.** ASub's synthetic default ctor emits `super()` at recompile,
but A's no-args ctor is `private` (because A is a `private static
class`). javac actually generates a synthetic accessor
`<init>(SyntheticMarker)` for cross-class private-ctor access; the
decompiler emits the unmediated `super()` form, which javac rejects
on access control: `OverloadsPeerClasses$A() has private access in
OverloadsPeerClasses$A`. Same defect on BSub.

**Candidate follow-up.** `dex-decompile-private-nested-class-ctor-accessor`
— when the parent class of a `super()` call is `private` and lives in
the SAME enclosing top-level class, route the call through javac's
synthetic-marker `<init>(<T>$N)` accessor (pattern: `<init>` whose
last param is a `*$1` synthetic class). Alternative: widen the
parent's ctor visibility on emit when a subclass needs the call —
less clean but simpler and equally correct.
