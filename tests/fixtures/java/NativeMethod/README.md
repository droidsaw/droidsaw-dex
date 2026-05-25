# NativeMethod

**Covers.** `native` method declaration (ACC_NATIVE 0x0100) on class
`A`, with subclass `B` that does not override. `main` guards the actual
invoke behind `args.length == 42` (always false at runtime), so no
JNI library is loaded. The corner under test is the IR / emit shape of
the native declaration, not its dynamic behavior.

**Status.** `compile_pass` — full round-trip clean.

**R8 source.** `src/test/examples/classmerging/ClassWithNativeMethodTest.java`
(Apache-2). Adapted by removing R8-only comment (`// Make sure that
A.method is not removed by tree shaking.`).

**Outcome.** Decompiler preserves `public native void method();` and
the empty body. javac re-accepts the declaration; runtime never invokes
it (the guard is false). Expected stdout is empty.

**Notes.** First fixture in the matrix to exercise ACC_NATIVE. Locks
the access-flag emission for native against any future ratchet drift.
