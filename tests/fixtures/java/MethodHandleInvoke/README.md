# MethodHandleInvoke

**Covers.** `MethodHandle.invokeExact(...)` lowered to DEX's
`invoke-polymorphic` opcode (`0xfa`) under `--min-api 26`. This is the
only canonical producer of `invoke-polymorphic` today — it carries a
per-call-site `proto_id` distinct from the target method's declared
signature, since `invokeExact`'s runtime type is `([Object)Object` but
the bytecode site wants the callsite-visible types.

**Status.** `compile_pass` (graduated 2026-04-23 by
`dex-decompile-invoke-polymorphic`).

**Fix shape.**

- `decode.rs` — new `InsnFormat::F45cc` + `F4rcc` variants (4 code
  units each; without these the parser misclassifies invoke-polymorphic
  as F35c's 3 units and silently drops the per-call-site proto_id).
  New decode arms route the pool slot to
  `PoolIndex::MethodAndProto(method, proto)`.
- `emit_dex.rs` — matching emit arms for round-trip emission, plus a
  `pool_proto_u16` helper.
- `types.rs` — `invoke_polymorphic_return_type` resolves the call-site
  proto's return type so `move-result-object` gets the correct type
  (e.g. `String` rather than `Object` or the surrounding register's
  leaked type). Parallel shape to `invoke_custom_return_type`.
- `emit.rs::emit_invoke_polymorphic` — emits
  `(ReturnType) receiver.method(args)` with the required source-level
  cast. Without the cast, javac rejects assignment to a non-Object
  LHS (signature-polymorphic contract).
- `emit.rs::pretty_class_name` + `emit_type` — rewrite `Outer$Inner`
  bytecode form to `Outer.Inner` source form for references inside
  `java.*` / `javax.*` / `kotlin*` namespaces, so
  `MethodHandles.Lookup` renders correctly (previously the decompiler
  emitted the illegal `MethodHandles$Lookup` type reference).
- `classes.rs` — `patch_throws_throwable_on_method_signatures` trigger
  broadened: beyond the pre-existing cross-catch-hoist pattern, any
  method body containing `MethodHandle.invoke*` / `findStatic`-family
  calls now also picks up `throws Throwable` on the enclosing method
  signature (required since those APIs throw checked exceptions).
