# LambdaCallSite

**Covers.** `d8 --min-api 26` preserves the canonical `invoke-custom` +
`call_site_ids` / `method_handles` shape for a non-capturing
`Supplier<String>` lambda. Under the fixture harness default (no
`--min-api`) d8 would lower this to a synthetic throw-stub class
(`LambdaCallSite$$ExternalSyntheticThrowRTE0`) that references
`"Instruction is unrepresentable in DEX V35: invoke-dynamic"`.

**Status.** `compile_fail` at the `Decompile` / `Recompile` stage until
LambdaMetafactory pattern reconstruction and emit-side `call_site_ids`
handling are implemented.

**Why the new fixture.** `Lambdas` is a broader test of three lambda
shapes (non-capturing `Runnable`, pure `Function`, capturing `Function`).
`LambdaCallSite` is a single-site minimal repro for the specific
`invoke-custom` lowering — useful for isolating emit / decompile gaps on
the bootstrap method + call_site_id surface without hiding the signal in
the broader fixture's noise.

