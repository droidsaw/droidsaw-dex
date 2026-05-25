# DualInterfaceSameSig

**Covers.** A single class implementing two interfaces that declare
the same method signature `void foo()`. JVMS resolves both interface
dispatches to the single concrete `InterfaceImpl.foo`. DEX
`invoke-interface` carries the receiver-type-as-declared-by-the-callsite,
so the two call sites (`a.foo()` with `a:A` vs `b.foo()` with `b:B`)
reference different `method_id`s even though they target the same body.

**Status.** `compile_pass` — full round-trip clean.

**R8 source.** `src/test/examples/classmerging/ConflictingInterfaceSignaturesTest.java`
(Apache-2). Adapted by stripping the R8-internal `@NeverInline escape`
helper (only relevant under R8's tree-shaking).

**Outcome.** Decompiler preserves the `implements A, B` clause + both
interface declarations. Both call sites lower correctly to
`invoke-interface` against the declared static type of the receiver.

**Notes.** First fixture to exercise `implements A, B` (multi-interface
on a single class) where the interfaces have identical method
signatures. Existing `Interface` fixture covers single-interface only.
