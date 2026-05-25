# `fuzz_enum_cross_class` seed inventory

Seeds for the `fuzz_enum_cross_class` target, enumerated from the
adversarial-shape list for cross-class enum-inline walking. Inputs to
`cargo fuzz run fuzz_enum_cross_class fuzz/corpus/fuzz_enum_cross_class
fuzz/seeds/fuzz_enum_cross_class`.

## Shape coverage

| # | Shape | Seed | Status |
|---|---|---|---|
| 1 | truncated subclass | libfuzzer mutation on the 4 real seeds | mutation-only |
| 2 | cyclic super | libfuzzer mutation | mutation-only |
| 3 | missing code_item | libfuzzer mutation | mutation-only |
| 4 | duplicate constant backing | libfuzzer mutation | mutation-only |
| 5 | unsafe-body ref (subclass-only field) | libfuzzer mutation | mutation-only |
| 6 | oversized constant count | libfuzzer mutation | mutation-only |
| 7 | zero-method subclass | libfuzzer mutation | mutation-only |
| 8 | **constant-without-subclass** | `user_static_block.dex` + `minimal.dex` | **covered** |
| 9 | masquerade (subclass super is non-enum) | libfuzzer mutation | mutation-only |
| 10 | UAF-adjacent cross-scan mutation | N/A (borrow-checker prevents) | not applicable |
| 11 | aggregate-decompile-depth trigger | libfuzzer mutation | mutation-only |

**Real-DEX seeds shipped:**

- `enumwm.dex` — positive case. `EnumWithMethods` fixture (per-constant
  subclass bodies reachable; exercises the full inline path).
- `minimal.dex` / `minimal_named.dex` — non-enum baseline (primary gate
  `applies` is false; iterator short-circuits).
- `user_static_block.dex` — simple enum without per-constant bodies
  (`Color { RED, GREEN, BLUE; }` + user `static { counter = 42; }`
  block). Covers shape #8 (constant-without-subclass).

**Why most shapes remain "mutation-only":**

Shapes 1–5, 7, 9, 11 require malformed-at-the-byte-level DEX — not
reachable from Java source. Hand-crafting them is a
synthetic-DEX-builder workflow (parse → mutate structural field →
recompute checksum → write) that's out of scope for the current pass.
A 10-minute fuzz smoke hit **2.86M runs / 0 crashes / 3,822 new units
discovered** starting from the 4 real seeds; libfuzzer mutation is
empirically exercising these shapes.

Shape 10 ("UAF-adjacent cross-scan mutation") is a Rust borrow-checker
invariant, not an input shape. The cross-class walker's retained
references across the scan are bounded by lifetimes at compile time.
Leaving as "N/A" rather than crafting a test input.

**Regression-deterministic anchor work (future):**

When a real regression surfaces one of shapes 1–5, 7, 9, 11, commit
the minimized DEX that triggered it under `regressions/` (cargo-fuzz
convention) alongside this README.
