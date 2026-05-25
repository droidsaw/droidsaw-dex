// Sealed-OBJECT `when` with 5 arms. Above the metadata-required threshold;
// recognizer fires.
//
// Audit evidence (Brief Status §1 finding #1):
//   per-arm: aload <discrim>
//          + getstatic <Sub>.INSTANCE
//          + invokestatic kotlin/jvm/internal/Intrinsics.areEqual:(LObject;LObject;)Z
//          + ifeq <next-arm>
//          + arm body
//          + goto <return>
//   default: new kotlin/NoWhenBranchMatchedException + dup + invokespecial + athrow
//
// Strong shared discriminator: NoWhenBranchMatchedException at fall-through.
// Java hand-written code never emits this exception type; single-symbol gate.

sealed class Color {
    object Red : Color()
    object Green : Color()
    object Blue : Color()
    object Yellow : Color()
    object Purple : Color()
}

fun describe(c: Color): String = when (c) {
    Color.Red -> "red"
    Color.Green -> "green"
    Color.Blue -> "blue"
    Color.Yellow -> "yellow"
    Color.Purple -> "purple"
}
