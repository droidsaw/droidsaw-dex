// data class destructuring. Audit finding #9: lowers to consecutive
// `componentN()` synthetic-method invokevirtuals on the same receiver,
// monotone N starting at 1. Single-shape; brief defers N>2 / lambda /
// `_`-discard variants per Open question §3 (RESOLVED 2026-04-30).

data class Pair2(val a: Int, val b: String)

fun useDestructure(p: Pair2): String {
    val (a, b) = p
    return "$a:$b"
}
