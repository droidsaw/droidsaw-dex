# StaticInitBlock

**Covers.** A `static { ... }` class-level initializer block. Dalvik emits this as a method named `<clinit>` (distinct from `<init>` which is the instance constructor); the JVM invokes `<clinit>` once per class, before any instance or static member is accessed.

**Status.** `compile_pass`.

**Fix.** Two compounding defects resolved in one pass:

1. **Declaration initializer suppression for `<clinit>`-assigned fields**: `static final` fields lacking an explicit `encoded_array_item` entry were getting a type-default initializer (`= 0`, `= null`, etc.). For fields that `<clinit>` assigns explicitly, that's a double-assign on `final` — javac rejects "cannot assign a value to final variable X". Fix: `classes.rs::collect_clinit_assigned_fields` scans the `<clinit>` method's instructions for SPut* opcodes targeting the current class, and `emit_fields` now skips the default-init fallback for fields in that set (leaving them as bare `static final int X;` declarations so `<clinit>` can assign them).

2. **Qualifier-free SPut emit from the declaring class**: the decompiler's SPut emission always rendered `{ClassName}.{field} = value` — but javac rejects qualified writes to `static final` fields even from inside the declaring class's own `<clinit>` block (unqualified `X = 10;` is required). Fix: `EmitCtx::own_class_desc` carries the currently-emitting class's type descriptor; `emit_expr`'s SPut arm compares against the target's class_idx and drops the qualifier when they match.

**Output form**:

```java
static final int X;
static final int Y;

static {
    X = 10;                              // unqualified, no double-assign
    int v0_1 = StaticInitBlock.X;        // SGet stays qualified — compiles fine
    Y = (v0_1 * 2);                      // unqualified
}
```

**Runtime**: `X=10` and `Y=20` on main's println, matching expected.txt.

**Out of scope.** SGet qualifier-stripping on same-class reads (current output qualifies reads like `StaticInitBlock.X` — valid Java, just verbose). Full `<clinit>` → declaration-initializer back-folding (emit `static final int X = 10;` instead of bare decl + static-block assign) — stylistic, not required for javac acceptance.
