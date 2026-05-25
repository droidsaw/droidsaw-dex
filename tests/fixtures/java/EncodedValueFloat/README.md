# EncodedValueFloat

**Covers.** DEX `VALUE_FLOAT` encoded_value variant across IEEE-754 f32 boundary values, via `static final float` constants. Uses `Float.floatToRawIntBits` so bit-pattern equality (not `==`) is the runtime check — NaN equality is preserved, ±0 distinguished, ±Inf distinguished.

| Field | Value | Bit pattern | Notes |
|---|---|---|---|
| `ZERO` | `+0.0f` | `0x00000000` | Positive zero. **May trigger zero-elision** on some variant of the bug. |
| `NEG_ZERO` | `-0.0f` | `0x80000000` | Negative zero — canonical sign-preservation edge. |
| `ONE` | `1.0f` | `0x3F800000` | Simple non-zero positive. |
| `NEG_ONE` | `-1.0f` | `0xBF800000` | Simple non-zero negative. |
| `MIN_NORMAL` | `Float.MIN_NORMAL` | `0x00800000` | Smallest normal f32. |
| `MAX` | `Float.MAX_VALUE` | `0x7F7FFFFF` | Largest finite f32. |
| `POS_INF` | `+∞` | `0x7F800000` | Positive infinity. |
| `NEG_INF` | `-∞` | `0xFF800000` | Negative infinity. |
| `NAN` | Quiet NaN | `0x7FC00000` | Canonical JVM qNaN. |

**Status.** Initially `compile_fail`. Expected defects on current emit:
- **Zero-elision**: `ZERO` (0.0f) or similar may lose its initializer if the static-values emitter treats `0.0f` bits (all-zero) as default.
- **Possible Float decode regression surface**: the `>> 32` issue in `read_uint_right_extend` narrowing has been fixed in tree. This fixture exercises the happy path + guards against any future reversion.
- **NaN canonicalization**: Java spec allows NaN representations to vary; using `floatToRawIntBits` and comparing exact bit patterns is the correct invariant. Different NaN payloads (`0x7FC00001` vs `0x7FC00000`) would surface as a ratchet mismatch.

**Why `floatToRawIntBits` not `==`**: `Float.NaN == Float.NaN` is `false` by IEEE-754 rule; `-0.0f == +0.0f` is `true` but `floatToRawIntBits` distinguishes them. Comparing bit patterns makes these edge cases first-class and ratchet-observable.

**Graduation condition.** Any defects surface → are fixed; when all resolved, fixture flips to `compile_pass`. If the only defect is zero-elision (shared with sibling fixtures), Float graduates alongside other zero-elision fixtures.

**Unique coverage vs siblings.** First fixture exercising the `VALUE_FLOAT` / `read_uint_right_extend` high-bit-packed encoded_value path. Siblings Byte/Short/Int/Long use `read_int` (low-bit-packed, sign-extended); Float/Double use `read_uint_right_extend` (high-bit-packed, zero-extended). Distinct code paths.
