# EncodedValueShort

**Covers.** DEX `VALUE_SHORT` encoded_value variant across 1-byte and 2-byte encoding paths, via `static final short` constants.

| Field | Value | Encoding | Notes |
|---|---|---|---|
| `ZERO` | `0` | (elided) | **Triggers zero-elision bug**. |
| `ONE_BYTE_POS` | `127` | 1-byte | High bit clear, no sign concerns. |
| `ONE_BYTE_NEG` | `-128` | 1-byte | Sign-extension path; short is signed per DEX spec so correctly decodes. |
| `TWO_BYTE_POS` | `0x1234` | 2-byte | 2-byte boundary crossed; little-endian byte order + sign-extension path. |
| `POS_MAX` | `Short.MAX_VALUE` (32767) | 2-byte | Max positive short. |
| `NEG_MAX` | `Short.MIN_VALUE` (-32768) | 2-byte | Max negative short; high-bit-set 2-byte case. |

**Status.** `compile_fail`. Single defect: zero-elision (same as siblings).

**Graduation condition.** Zero-elision fix lands.

**Unique coverage vs siblings.** Short is the smallest type exercising the **2-byte encoded_value path**. If a future bug affects only the multi-byte size-packing logic (not the 1-byte fast path), this fixture is the first to surface it.
