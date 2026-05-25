# BooleanAndNullReturns

Exercises return-position type inference for const-zero defs.

## What's being tested

DEX bytecode lowers `false` / `null` literals to `const/4 0` followed by
`return` / `return-object`. Without consulting the return type, the
decompiler types the const def as
`Int` (TypeEnv default) because `resolve_one_const` did not consult
`Return` / `ReturnObject` use sites when refining const-def types. With
`var_ty == Int`, `emit_expr`'s null-fold (`literal == 0 && ty.is_reference()`)
and Boolean-fold (`emit_literal(0, Boolean) → "false"`) never fired, so
both `return false;` and `return null;` emitted as `return 0;` — invalid
Java in both contexts.

The fix plumbs `method_idx` through `infer_types` →
`resolve_consts` → `resolve_one_const` and adds a `Return | ReturnObject`
arm that returns the method's declared return type. With sound typing,
the existing emit paths render `false` / `true` / `null` correctly with
no further emit-side changes.

## Why this fixture covers the bug

- `alwaysFalse()` → `boolean` return + `const/4 0` → `return false;`
- `alwaysTrue()` → `boolean` return + `const/4 1` → `return true;`
- `alwaysNull()` → `Object` return + `const/4 0` → `return null;`
- `maybeNull(int)` → `String` return + control-flow with both null and
  non-null returns; ensures the fix doesn't regress non-null reference
  returns or the return-from-conditional-branch case.

The runtime stdout (`expected.txt`) of the original and the
decompiled-then-recompiled program must match exactly.
