// Sealed-class `when` over a sealed root with subclass subtypes (instanceof
// lowering). 2 arms — boundary case where kotlinc-1.9.22 may degrade the
// recognizer's `instanceof`-chain to byte-identical Java hand-written code.
// Per Brief Open question §6 (RESOLVED 2026-04-30): recognizer rejects 2-arm
// shapes without sealed-root metadata visibility. This fixture is included
// for negative-coverage of that gate (corpus compiles; recognizer does NOT
// fire on it because the sealed-root metadata threshold isn't met at 2
// arms — confirmed once the recognizer lands).

sealed class Result2 {
    class Ok(val value: Int) : Result2()
    class Err(val msg: String) : Result2()
}

fun describe(r: Result2): String = when (r) {
    is Result2.Ok -> "ok=${r.value}"
    is Result2.Err -> "err=${r.msg}"
}
