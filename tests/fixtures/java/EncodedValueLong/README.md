# EncodedValueLong

**Covers.** DEX `VALUE_LONG` encoded_value variant across 1/4/5/8-byte encoding paths, via `static final long` constants.

| Field | Value | Encoding | Notes |
|---|---|---|---|
| `ZERO` | `0L` | (elided) | **Triggers zero-elision bug**. |
| `ONE_BYTE` | `127L` | 1-byte | Small values fit in 1 byte. |
| `FOUR_BYTE` | `0x12345678L` | 4-byte | Values that fit in 32 bits but go through the long encoded_value path. |
| `FIVE_BYTE` | `0x123456789L` | 5-byte | **First fixture exercising the 5-byte encoded_value path** (value requiring bit 32 set). Only `VALUE_LONG` can reach size=5. |
| `EIGHT_BYTE` | `0x1234567890ABCDEFL` | 8-byte | Max-width encoded_value; full 8-byte path. |
| `POS_MAX` | `Long.MAX_VALUE` | 8-byte | Maximum positive long. |
| `NEG_ONE` | `-1L` | 1-byte | Sign-extension on the shortest form; long is signed per DEX spec. |
| `NEG_MAX` | `Long.MIN_VALUE` | 8-byte | Maximum negative long; full 8-byte with high bit set. |

**Status.** `compile_fail`. Single defect: zero-elision.

**Graduation condition.** Zero-elision fix lands.

**Unique coverage vs siblings.** Long exercises the 5-byte and 8-byte encoding paths that no other primitive variant reaches (Int caps at 4-byte). The `encoded_value size_arg` field is 3 bits — `[0,7]` → `size ∈ [1,8]`. Size=5/6/7/8 are Long-only. If a future `size ≥ 5` handling bug surfaces (e.g., a regression in `read_int`'s loop bound), this fixture catches it.

Also: `NEG_MAX` (0x8000000000000000) is the canonical overflow-sensitive constant — if any new `i64 → usize`-flavored cast slips past the arithmetic-hardening ratchet, this value is the one most likely to trigger it.
