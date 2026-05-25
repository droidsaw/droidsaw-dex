# ArrayOverload

**Covers.** Method-overload resolution on covariant array parameter
types (`method(A[])` vs `method(B[])` with `B extends A`). Java array
covariance (`B[] <: A[]`) makes the static type at the call site
load-bearing.

**Status.** `compile_fail` — recompile stage fails.

**R8 source.** `src/test/examples/classmerging/ArrayTypeCollisionTest.java`
(Apache-2). Adapted by removing R8-only comments (`// A cannot be
merged into B because that would lead to a collision.`); no other
structural changes.

**Outcome.** Decompiler emits

```
String v3_7 = "In method(A[]), length: " + v3_5;
```

referencing `v3_5` without ever declaring it. The undeclared SSA temp
holds the result of an `arraylength` field-read on `obj`. javac rejects:
`cannot find symbol — variable v3_5`. Same general class of defect as
`ExceptionHierarchy` (catch-binding temp), but the trigger is on the
`array.length` path inside a string concat rather than a catch-binding.

**Candidate follow-up.** `dex-decompile-array-length-tmpvar-decl` — emit
a `<Type> v<N> = ...;` declaration on the first def of an SSA temp from
the `array-length` (`opcode 0x21`) instruction. Likely shares an
implementation with `dex-decompile-catch-tmpvar-decl`; both surface a
missing var-decl emit pass for SSA-named temporaries that don't appear
in javac's local variable table.
