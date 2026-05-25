// Sealed-OBJECT `when` with 2 arms. Lower-bound case for the recognizer's
// metadata-required threshold (Brief Open question §6, RESOLVED 2026-04-30).
// At 2 arms the recognizer rejects without sealed-root metadata visibility
// (false-positive risk on Java hand-written `==` chains is too high). This
// fixture exists for negative-coverage of the threshold gate.

sealed class Switch {
    object On : Switch()
    object Off : Switch()
}

fun describe(s: Switch): String = when (s) {
    Switch.On -> "on"
    Switch.Off -> "off"
}
