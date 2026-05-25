# EncodedValueByte

**Covers.** DEX `VALUE_BYTE` encoded_value variant via `static final byte` constants.

| Field | Value | Notes |
|---|---|---|
| `ZERO` | `0` | **Triggers zero-elision bug** (see below). |
| `POS_MAX` | `127` | `Byte.MAX_VALUE`; 1-byte encoding, high bit clear. |
| `NEG_ONE` | `-1` | Sign-extension check (0xFF stored → `i8 = -1`). |
| `NEG_MAX` | `-128` | `Byte.MIN_VALUE`; 1-byte, high bit set, sign-extended correctly (byte IS signed per DEX spec — `read_int` is correct for this variant). |

**Status.** `compile_fail`. Single defect: `public static final byte ZERO;` is emitted without an initializer. Same root cause as EncodedValueChar's MIN field — `encoded_array_item` elides trailing zero-valued entries per DEX spec §VII.3.1, and our static-values emitter doesn't recover the "default-to-zero" reconstruction.

**Graduation condition.** Zero-elision fix lands → fixture flips to `compile_pass`. Expected to graduate alongside EncodedValueShort / EncodedValueInt / EncodedValueLong (shared fix).

**Why this fixture exists.** Byte is signed per DEX spec (encoded with `read_int`), so this fixture isn't catching a currently-known sign-extension bug — it defends against regression. Any future refactor that breaks Byte's sign-extend path (e.g., switching to `read_uint` by analogy with a future CHAR fix without noting the sign difference) is caught at ratchet time.
