# EncodedValueInt

**Covers.** DEX `VALUE_INT` encoded_value variant across 1-byte through 4-byte encoding paths, via `static final int` constants.

| Field | Value | Encoding | Notes |
|---|---|---|---|
| `ZERO` | `0` | (elided) | **Triggers zero-elision bug**. |
| `ONE_BYTE` | `127` | 1-byte | High bit clear. |
| `TWO_BYTE` | `0x1234` | 2-byte | 2-byte boundary. |
| `THREE_BYTE` | `0x123456` | 3-byte | **First fixture exercising the 3-byte encoded_value path** — covers middle-width bugs that would only surface at size=3 (value range 0x8000..0x7FFFFF positive, sign-mirror negative). |
| `FOUR_BYTE` | `0x12345678` | 4-byte | Max positive 4-byte. |
| `POS_MAX` | `Integer.MAX_VALUE` | 4-byte | Full int range. |
| `NEG_ONE` | `-1` | 1-byte | Sign-extension on the shortest form. |
| `NEG_MAX` | `Integer.MIN_VALUE` | 4-byte | High-bit-set max-width. |

**Status.** `compile_fail`. Single defect: zero-elision.

**Graduation condition.** Zero-elision fix lands.

**Unique coverage vs siblings.** Int exercises the 3-byte encoding path that neither Byte (1-byte only), Short (1-byte or 2-byte), nor Long (skips 3-byte for sensible values) exercises. DEX spec allows `VALUE_INT size ∈ [1,4]`; size=3 is the often-overlooked middle width.
