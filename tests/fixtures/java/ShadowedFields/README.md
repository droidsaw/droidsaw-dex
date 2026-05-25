# ShadowedFields

**Covers.** B extends A with both declaring a field named `obj` (A.obj
of type A, B.obj of type B). JVMS keeps both fields physically — there
is no override for fields, the discriminator is the declaring class.
The cast `((B) super.obj).message` in B.toString explicitly reads A's
field via super, then casts the value back to B to project `.message`.

**Status.** `compile_fail` — recompile stage fails.

**R8 source.** `src/test/examples/classmerging/FieldCollisionTest.java`
(Apache-2). Adapted by removing the R8 `// Will be merged into B.`
comment.

**Outcome.** Decompiler emits B's no-args ctor as

```java
public B() {
    super(null);
}
```

but A has only a no-args default ctor (no `(String)` ctor). javac
rejects: `constructor ShadowedFields$A in class ShadowedFields$A
cannot be applied to given types; required: no arguments; found: <null>`.

The R8 source had B() emit `this(null)` which delegates to the
`B(String)` ctor; that ctor in turn calls the implicit default `super()`.
The decompiler is collapsing this two-step pattern into a single
direct `super(null)` call, conflating `<init>`-on-same-class with
`<init>`-on-super-class.

**Candidate follow-up.** `dex-decompile-this-vs-super-ctor-call` — at
the emit boundary, when an `<init>` body's first instruction is
`invoke-direct` on `<init>` of the SAME class as the enclosing
`<init>`, emit `this(args)`; when it targets the IMMEDIATE SUPER
class, emit `super(args)`. DEX has no separate opcode — both lower to
`invoke-direct` on `<init>` of the appropriate class — so the
discriminator is the target's class identity vs the enclosing
constructor's class identity.
