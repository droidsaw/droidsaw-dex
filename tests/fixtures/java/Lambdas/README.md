# Lambdas

**Covers.** Java 8 `LambdaMetafactory` lowering via DEX `invoke-custom`:
one non-capturing `Runnable`, one pure `Function<Integer, Integer>`, one
`Function` that captures an enclosing local (`int c`) via the
bootstrap's `invokedType` proto.

**Status.** `compile_pass` (graduated 2026-04-23 by `dex-decompile-lambdas`).

**Fix shape.** `fixture-harness-d8-min-api-per-fixture` set `d8_min_api
= 26` on this fixture so d8 preserves the canonical invoke-custom shape
instead of desugaring to a `Lambdas$$ExternalSyntheticThrowRTE0` stub.
`dex-decompile-lambdas` then added:

- Map-list-driven parsing of the `TYPE_CALL_SITE_ID_ITEM` (0x0007) and
  `TYPE_METHOD_HANDLE_ITEM` (0x0008) sections in `parser.rs`.
- `VALUE_METHOD_TYPE` / `VALUE_METHOD_HANDLE` variants on
  `EncodedValue` so the call-site encoded_array parses end-to-end.
- `PoolIndex::CallSite(CallSiteIdx)` so `decode.rs` stops mis-classifying
  invoke-custom's operand as a method id (without this, emits look
  like `10.lambda$main$1()` — the captured `c` becomes a receiver via
  pool-index collision).
- `emit_invoke_custom` in `emit.rs` — pattern-matches LambdaMetafactory
  bootstraps, reads the impl method-handle out of call_site[4], and
  lowers to an explicit lambda expression
  (`_a0 -> Lambdas.dsaw$lambda$main$1((Integer) _a0)`) with per-SAM-arg
  casts so javac's raw-erasure type check is satisfied.
- `sanitize_id` rewrites `lambda$<...>` → `dsaw$lambda$<...>` so the
  decompiled synthetic methods don't collide with javac's own
  invokedynamic-synthesized naming on recompile.
- SSA type inference (`types.rs`) wires a parallel
  `prev_call_site_return` channel so `move-result-object` after an
  invoke-custom inherits the functional-interface class rather than
  falling through to `Bottom` (which previously produced
  `int v2 = Lambdas::...`).
