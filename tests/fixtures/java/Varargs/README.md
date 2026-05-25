# Varargs

**Covers.** A method with a varargs trailing parameter: `void show(String label, int... nums)`. Dalvik lowers the call site to `array-new` + element stores + `invoke-virtual`, and sets `ACC_VARARGS` (0x80) in the method's access flags.

**Status.** `compile_pass` (graduated by the `dex-decompile-varargs` stream).

**Fix.** The earlier `compile_fail` was caused by `emit_access_flags` (shared function, no context) rendering the 0x80 bit as `transient` — the field-only interpretation. On methods, 0x80 is `ACC_VARARGS`, which surfaces at source level as the `int...` parameter syntax, NOT as a modifier keyword. Likewise 0x40 is `ACC_BRIDGE` (synthetic, not source-emitted) on methods but `ACC_VOLATILE` on fields — same shared-bit problem.

Fix: both method callers (`classes.rs::emit_abstract_method` + `emit.rs::emit_method`) now mask out bits 0x40 and 0x80 before passing to `emit_access_flags`. The method signature loses the spurious `transient` / `volatile` keywords.

**Output form**:

```java
static void show(String v4, int[] v5) { ... }   // no "transient"
```

The method signature is NOT restored to the `int...` sugar form — it stays as explicit `int[]`. Javac accepts both call-site forms (`show("a", v0)` where v0 is `int[]` compiles against `show(String, int...)` and also against `show(String, int[])`), so the fixture compiles + runs correctly with either signature form. Full sugar restoration (read ACC_VARARGS + rewrite last param's type + reconstruct variadic call sites) is a future polish stream — `dex-decompile-varargs-sugar` — not required for fixture graduation.

**Out of scope.** Call-site reconstruction (`show("a", 1, 2, 3)` from array-fill sequences); `int...` signature sugar; full ACC_BRIDGE synthetic-method elision.
