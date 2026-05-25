// when (Int) — 2 arms (dense range). Audit finding #8: kotlinc-1.9.22
// emits `tableswitch` consistently for dense ranges — no threshold migration.

fun describe(i: Int): String = when (i) {
    1 -> "v1"
    2 -> "v2"
    else -> "other"
}
