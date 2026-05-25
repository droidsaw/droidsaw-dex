# ExceptionHierarchy

**Covers.** Catch-arm ordering on a 4-class exception subclass chain
(`ExceptionA ← ExceptionB`, `Exception1 ← Exception2`). Two throw sites
(`throwExceptionB`, `throwException2`) keep all four catch arms reachable
per JLS. The runtime resolves to the most-specific arm, not the broadest.

**Status.** `compile_fail` — recompile stage fails.

**R8 source.** `src/test/examples/classmerging/ExceptionTest.java`
(Apache-2). Adapted by adding a second throw site so all four catch
arms remain reachable per JLS, and stripping R8-internal `@NeverInline`
helpers (none in this source).

**Outcome.** Decompiler emits the catch-binding's StringBuilder concat
chain as

```
v1 = new java.lang.StringBuilder();
v1 = v1.append("Caught Exception" + ...);
```

without ever declaring `v1`. javac then rejects: `cannot find symbol —
variable v1`. The defect is in the SSA-temp emission pass: a freshly
introduced StringBuilder temp inside a catch block doesn't get a leading
`StringBuilder` type declaration.

**Candidate follow-up.** `dex-decompile-catch-tmpvar-decl` — emit a
`<Type> v<N> = ...;` declaration on the first def of an SSA temp inside
a catch-block scope (or hoist the decl to the first def in any block,
not just catch-bodies — see sibling `dex-decompile-array-length-tmpvar-decl`).
