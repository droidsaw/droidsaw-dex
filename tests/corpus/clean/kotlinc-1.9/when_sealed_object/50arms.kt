// Sealed-OBJECT `when` at 50-arm scale. Audit evidence: linear
// `Intrinsics.areEqual + getstatic INSTANCE` chain — NO threshold migration.
// Recognizer shape is the same as 5arms-object; same as MYB.A00 lowering.

sealed class Sig {
    object S1 : Sig()
    object S2 : Sig()
    object S3 : Sig()
    object S4 : Sig()
    object S5 : Sig()
    object S6 : Sig()
    object S7 : Sig()
    object S8 : Sig()
    object S9 : Sig()
    object S10 : Sig()
    object S11 : Sig()
    object S12 : Sig()
    object S13 : Sig()
    object S14 : Sig()
    object S15 : Sig()
    object S16 : Sig()
    object S17 : Sig()
    object S18 : Sig()
    object S19 : Sig()
    object S20 : Sig()
    object S21 : Sig()
    object S22 : Sig()
    object S23 : Sig()
    object S24 : Sig()
    object S25 : Sig()
    object S26 : Sig()
    object S27 : Sig()
    object S28 : Sig()
    object S29 : Sig()
    object S30 : Sig()
    object S31 : Sig()
    object S32 : Sig()
    object S33 : Sig()
    object S34 : Sig()
    object S35 : Sig()
    object S36 : Sig()
    object S37 : Sig()
    object S38 : Sig()
    object S39 : Sig()
    object S40 : Sig()
    object S41 : Sig()
    object S42 : Sig()
    object S43 : Sig()
    object S44 : Sig()
    object S45 : Sig()
    object S46 : Sig()
    object S47 : Sig()
    object S48 : Sig()
    object S49 : Sig()
    object S50 : Sig()
}

fun describe(s: Sig): String = when (s) {
    Sig.S1 -> "s1"
    Sig.S2 -> "s2"
    Sig.S3 -> "s3"
    Sig.S4 -> "s4"
    Sig.S5 -> "s5"
    Sig.S6 -> "s6"
    Sig.S7 -> "s7"
    Sig.S8 -> "s8"
    Sig.S9 -> "s9"
    Sig.S10 -> "s10"
    Sig.S11 -> "s11"
    Sig.S12 -> "s12"
    Sig.S13 -> "s13"
    Sig.S14 -> "s14"
    Sig.S15 -> "s15"
    Sig.S16 -> "s16"
    Sig.S17 -> "s17"
    Sig.S18 -> "s18"
    Sig.S19 -> "s19"
    Sig.S20 -> "s20"
    Sig.S21 -> "s21"
    Sig.S22 -> "s22"
    Sig.S23 -> "s23"
    Sig.S24 -> "s24"
    Sig.S25 -> "s25"
    Sig.S26 -> "s26"
    Sig.S27 -> "s27"
    Sig.S28 -> "s28"
    Sig.S29 -> "s29"
    Sig.S30 -> "s30"
    Sig.S31 -> "s31"
    Sig.S32 -> "s32"
    Sig.S33 -> "s33"
    Sig.S34 -> "s34"
    Sig.S35 -> "s35"
    Sig.S36 -> "s36"
    Sig.S37 -> "s37"
    Sig.S38 -> "s38"
    Sig.S39 -> "s39"
    Sig.S40 -> "s40"
    Sig.S41 -> "s41"
    Sig.S42 -> "s42"
    Sig.S43 -> "s43"
    Sig.S44 -> "s44"
    Sig.S45 -> "s45"
    Sig.S46 -> "s46"
    Sig.S47 -> "s47"
    Sig.S48 -> "s48"
    Sig.S49 -> "s49"
    Sig.S50 -> "s50"
}
