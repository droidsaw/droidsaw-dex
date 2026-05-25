# EncodedValueDouble

**Covers.** DEX `VALUE_DOUBLE` encoded_value variant across IEEE-754 f64 boundary values, via `static final double` constants. Uses `Double.doubleToRawLongBits` so bit-pattern equality (not `==`) is the runtime check.

| Field | Value | Bit pattern | Notes |
|---|---|---|---|
| `ZERO` | `+0.0` | `0x0000000000000000` | Positive zero — zero-elision candidate. |
| `NEG_ZERO` | `-0.0` | `0x8000000000000000` | Negative zero. |
| `ONE` | `1.0` | `0x3FF0000000000000` | Simple non-zero positive. |
| `NEG_ONE` | `-1.0` | `0xBFF0000000000000` | Simple non-zero negative. |
| `MIN_NORMAL` | `Double.MIN_NORMAL` | `0x0010000000000000` | Smallest normal f64. |
| `MAX` | `Double.MAX_VALUE` | `0x7FEFFFFFFFFFFFFF` | Largest finite f64. |
| `POS_INF` | `+∞` | `0x7FF0000000000000` | Positive infinity. |
| `NEG_INF` | `-∞` | `0xFFF0000000000000` | Negative infinity. |
| `NAN` | Quiet NaN | `0x7FF8000000000000` | Canonical JVM qNaN. |

**Status.** Initially `compile_fail`. Expected defects mirror EncodedValueFloat: zero-elision on ZERO + possible encoded_value high-bit-packing path reversion.

**Unique coverage vs siblings.** Exercises the 8-byte `read_uint_right_extend` path. Combined with EncodedValueLong (8-byte signed), covers both 8-byte encoded_value flavors.

**Why `doubleToRawLongBits` not `==`**: same rationale as EncodedValueFloat — IEEE-754 NaN-equality is false, ±0 are `==` but bit-distinct; raw bits are the correct invariant for round-trip testing.
