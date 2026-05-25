// when (String) — 5 arms. Audit finding #7: kotlinc-1.9.22 switches lowering
// strategy between 2 and 5 arms — emits `String.hashCode() + tableswitch`.
// Per-bucket: `aload + ldc "key" + invokevirtual String.equals + ifne body`.

fun classify(s: String): Int = when (s) {
    "k1" -> 1
    "k2" -> 2
    "k3" -> 3
    "k4" -> 4
    "k5" -> 5
    else -> -1
}
