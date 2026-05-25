// when (Int) — 5 arms (dense range). Audit finding #8: kotlinc-1.9.22
// emits `tableswitch` consistently for dense ranges — no threshold migration.

fun describe(i: Int): String = when (i) {
    1 -> "v1"
    2 -> "v2"
    3 -> "v3"
    4 -> "v4"
    5 -> "v5"
    else -> "other"
}
