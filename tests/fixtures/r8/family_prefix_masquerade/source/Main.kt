// Adversarial PoC: namespace-squat under the `androidx.*` family prefix.
//
// Why this lives in the repo: self-contained proof that the
// family-prefix filter is an FP-reduction hint, not a security
// boundary. Permanent reminder that the family list isn't a safety
// guarantee — here's a working bypass.
//
// The threat model is documented on the `KNOWN_FP_FAMILY` const in
// tests/r8_fdroid_apk_sweep.rs. Summary: in mapping-less sweeps the
// recogniser fires on outline-shape methods (I4–I13) regardless of
// class name; the family prefix check suppresses the marker as "looks
// like legitimate library code." An attacker controlling DEX class
// names can name a malicious helper with a family prefix and satisfy
// the structural invariants to slip past the FP filter.
//
// DEX has no namespace enforcement: `androidx.*`, `j$/*`, `io.flutter.*`,
// and `h1*` are owned by the Android/R8/Flutter toolchains by
// CONVENTION, not by runtime restriction. A direct DEX toolchain
// (smali, raw bytecode emit) — or, as here, a Kotlin compiler given
// `package androidx.adversarial.poc` — produces a class whose FQCN
// starts with `androidx.` regardless of any AndroidX provenance.
//
// This fixture demonstrates the masquerade end-to-end:
//
//   1. A class `androidx.adversarial.poc.Backdoor` with a single
//      static helper `execute(int, String)` whose body is straight-line
//      and short (satisfies I4, I6, I7, I8, I13).
//   2. 25 caller methods in `androidx.adversarial.poc.Main`, each
//      calling `Backdoor.execute` from a straight-line body. This
//      satisfies I9 (≥ 20 distinct callers).
//
// The ratchet test asserts:
//
//   * Recogniser fires on at least one marker whose class name starts
//     with `androidx.` (lexical masquerade succeeds).
//   * `is_in_known_fp_family()` returns true → would be suppressed in
//     mapping-less sweep, contributing to `androidx` bucket in
//     `namespace_rollup`.
//   * `classify_synthetic_kind()` returns Unknown → ZERO structural
//     attestation. The marker contributes 0 to `namespace_rollup_attested`.
//
// Both the namespace_rollup classification AND the attestation rate
// are wrong from a security standpoint: the marker IS suspect, but
// the FP filter hides it.
//
// The mitigation is mapping-paired analysis: the `mapping.txt` LHS
// (original class name) under R8's own toolchain would NOT carry the
// `com.android.tools.r8.outline` annotation on Backdoor (because R8
// did not synthesize it; the attacker did). In mapping-less analysis,
// no such ground truth exists — the masquerade succeeds.
//
// The "obvious" part: the class is literally named `Backdoor` and the
// helper returns the string "PWNED:<i>:<s>". Anyone reading the
// decompiled output sees it immediately; only the SUMMARY statistics
// in the mapping-less sweep mask it.

package androidx.adversarial.poc

object Backdoor {
    @JvmStatic
    fun execute(i: Int, s: String): String = "PWNED:$i:$s"
}

// 25 caller methods, each calling Backdoor.execute from a straight-line
// body. The arity, body size, and caller count satisfy the recogniser's
// I4–I13 structural invariants by construction.
class Main {
    fun call01() { println(Backdoor.execute(1, "a")) }
    fun call02() { println(Backdoor.execute(2, "b")) }
    fun call03() { println(Backdoor.execute(3, "c")) }
    fun call04() { println(Backdoor.execute(4, "d")) }
    fun call05() { println(Backdoor.execute(5, "e")) }
    fun call06() { println(Backdoor.execute(6, "f")) }
    fun call07() { println(Backdoor.execute(7, "g")) }
    fun call08() { println(Backdoor.execute(8, "h")) }
    fun call09() { println(Backdoor.execute(9, "i")) }
    fun call10() { println(Backdoor.execute(10, "j")) }
    fun call11() { println(Backdoor.execute(11, "k")) }
    fun call12() { println(Backdoor.execute(12, "l")) }
    fun call13() { println(Backdoor.execute(13, "m")) }
    fun call14() { println(Backdoor.execute(14, "n")) }
    fun call15() { println(Backdoor.execute(15, "o")) }
    fun call16() { println(Backdoor.execute(16, "p")) }
    fun call17() { println(Backdoor.execute(17, "q")) }
    fun call18() { println(Backdoor.execute(18, "r")) }
    fun call19() { println(Backdoor.execute(19, "s")) }
    fun call20() { println(Backdoor.execute(20, "t")) }
    fun call21() { println(Backdoor.execute(21, "u")) }
    fun call22() { println(Backdoor.execute(22, "v")) }
    fun call23() { println(Backdoor.execute(23, "w")) }
    fun call24() { println(Backdoor.execute(24, "x")) }
    fun call25() { println(Backdoor.execute(25, "y")) }

    companion object {
        @JvmStatic
        fun main(args: Array<String>) {
            val m = Main()
            m.call01(); m.call02(); m.call03(); m.call04(); m.call05()
            m.call06(); m.call07(); m.call08(); m.call09(); m.call10()
            m.call11(); m.call12(); m.call13(); m.call14(); m.call15()
            m.call16(); m.call17(); m.call18(); m.call19(); m.call20()
            m.call21(); m.call22(); m.call23(); m.call24(); m.call25()
        }
    }
}
