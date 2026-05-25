// when (Int) — sparse values. Audit finding #8: sparse non-contiguous values
// (1, 100, 1000, 10000) trigger `lookupswitch` — distinct lowering from dense.
// Required for recognizer roundtrip-gate coverage (was missing from initial Brief).

fun describe(i: Int): String = when (i) {
    1 -> "v1"
    100 -> "v100"
    1000 -> "v1000"
    10000 -> "v10000"
    else -> "other"
}
