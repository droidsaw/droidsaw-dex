// when (String) — 2 arms. Audit finding #7: kotlinc-1.9.22 emits a linear
// `Intrinsics.areEqual` chain at 2 arms (NOT hashCode+switch) — IDENTICAL
// primitive to sealed-OBJECT lowering. Recognizer shares code with sealed-object.

fun classify(s: String): Int = when (s) {
    "yes" -> 1
    "no" -> 0
    else -> -1
}
