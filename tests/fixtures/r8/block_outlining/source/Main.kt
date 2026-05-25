// Engineered Kotlin source for the R8 BlockOutline recogniser
// fixture. THREE caller groups, each invoking an identical 4-
// instruction StringBuilder sequence but with a DIFFERENT
// parameter type (Int, Long, String). R8's outliner extracts each
// body signature into its own synthetic helper because the
// underlying invoke targets differ per type
// (`StringBuilder.append(int)` vs `.append(long)` vs `.append(String)`),
// yielding distinct outlined-method bodies — three outline body
// annotations in the mapping.
//
// Each group has 30 distinct caller methods, comfortably above
// R8's OutlineOptions.threshold = 20 distinct callers per body.
// Each caller satisfies the I4-I13 invariants:
//   I4 (ACC_PUBLIC | ACC_STATIC) — R8 emits the helpers with these.
//   I6 (straight-line) — only invoke* + new-instance, no branches.
//   I7 (3-99 bytes)   — ~4 invokes + return = well within bounds.
//   I8 (arity <= 5)   — single typed param.
//   I9 (>= 20 distinct callers) — 30 callers per group.
//   I13 (param_count >= 1) — single typed param.
//
// Each caller appends a different constant suffix so R8 doesn't
// fold callers via deduplication; the extracted body takes the
// constant as a third helper param.

package fox.droidsaw.r8fixture

object GroupAInt {
    @JvmStatic fun a01(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|01"); return sb.toString() }
    @JvmStatic fun a02(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|02"); return sb.toString() }
    @JvmStatic fun a03(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|03"); return sb.toString() }
    @JvmStatic fun a04(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|04"); return sb.toString() }
    @JvmStatic fun a05(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|05"); return sb.toString() }
    @JvmStatic fun a06(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|06"); return sb.toString() }
    @JvmStatic fun a07(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|07"); return sb.toString() }
    @JvmStatic fun a08(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|08"); return sb.toString() }
    @JvmStatic fun a09(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|09"); return sb.toString() }
    @JvmStatic fun a10(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|10"); return sb.toString() }
    @JvmStatic fun a11(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|11"); return sb.toString() }
    @JvmStatic fun a12(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|12"); return sb.toString() }
    @JvmStatic fun a13(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|13"); return sb.toString() }
    @JvmStatic fun a14(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|14"); return sb.toString() }
    @JvmStatic fun a15(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|15"); return sb.toString() }
    @JvmStatic fun a16(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|16"); return sb.toString() }
    @JvmStatic fun a17(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|17"); return sb.toString() }
    @JvmStatic fun a18(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|18"); return sb.toString() }
    @JvmStatic fun a19(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|19"); return sb.toString() }
    @JvmStatic fun a20(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|20"); return sb.toString() }
    @JvmStatic fun a21(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|21"); return sb.toString() }
    @JvmStatic fun a22(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|22"); return sb.toString() }
    @JvmStatic fun a23(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|23"); return sb.toString() }
    @JvmStatic fun a24(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|24"); return sb.toString() }
    @JvmStatic fun a25(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|25"); return sb.toString() }
    @JvmStatic fun a26(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|26"); return sb.toString() }
    @JvmStatic fun a27(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|27"); return sb.toString() }
    @JvmStatic fun a28(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|28"); return sb.toString() }
    @JvmStatic fun a29(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|29"); return sb.toString() }
    @JvmStatic fun a30(x: Int): String { val sb = StringBuilder(); sb.append("A:"); sb.append(x); sb.append("|30"); return sb.toString() }
}

object GroupBLong {
    @JvmStatic fun b01(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|01"); return sb.toString() }
    @JvmStatic fun b02(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|02"); return sb.toString() }
    @JvmStatic fun b03(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|03"); return sb.toString() }
    @JvmStatic fun b04(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|04"); return sb.toString() }
    @JvmStatic fun b05(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|05"); return sb.toString() }
    @JvmStatic fun b06(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|06"); return sb.toString() }
    @JvmStatic fun b07(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|07"); return sb.toString() }
    @JvmStatic fun b08(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|08"); return sb.toString() }
    @JvmStatic fun b09(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|09"); return sb.toString() }
    @JvmStatic fun b10(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|10"); return sb.toString() }
    @JvmStatic fun b11(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|11"); return sb.toString() }
    @JvmStatic fun b12(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|12"); return sb.toString() }
    @JvmStatic fun b13(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|13"); return sb.toString() }
    @JvmStatic fun b14(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|14"); return sb.toString() }
    @JvmStatic fun b15(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|15"); return sb.toString() }
    @JvmStatic fun b16(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|16"); return sb.toString() }
    @JvmStatic fun b17(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|17"); return sb.toString() }
    @JvmStatic fun b18(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|18"); return sb.toString() }
    @JvmStatic fun b19(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|19"); return sb.toString() }
    @JvmStatic fun b20(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|20"); return sb.toString() }
    @JvmStatic fun b21(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|21"); return sb.toString() }
    @JvmStatic fun b22(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|22"); return sb.toString() }
    @JvmStatic fun b23(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|23"); return sb.toString() }
    @JvmStatic fun b24(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|24"); return sb.toString() }
    @JvmStatic fun b25(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|25"); return sb.toString() }
    @JvmStatic fun b26(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|26"); return sb.toString() }
    @JvmStatic fun b27(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|27"); return sb.toString() }
    @JvmStatic fun b28(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|28"); return sb.toString() }
    @JvmStatic fun b29(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|29"); return sb.toString() }
    @JvmStatic fun b30(x: Long): String { val sb = StringBuilder(); sb.append("B:"); sb.append(x); sb.append("|30"); return sb.toString() }
}

object GroupCStr {
    @JvmStatic fun c01(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|01"); return sb.toString() }
    @JvmStatic fun c02(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|02"); return sb.toString() }
    @JvmStatic fun c03(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|03"); return sb.toString() }
    @JvmStatic fun c04(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|04"); return sb.toString() }
    @JvmStatic fun c05(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|05"); return sb.toString() }
    @JvmStatic fun c06(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|06"); return sb.toString() }
    @JvmStatic fun c07(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|07"); return sb.toString() }
    @JvmStatic fun c08(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|08"); return sb.toString() }
    @JvmStatic fun c09(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|09"); return sb.toString() }
    @JvmStatic fun c10(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|10"); return sb.toString() }
    @JvmStatic fun c11(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|11"); return sb.toString() }
    @JvmStatic fun c12(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|12"); return sb.toString() }
    @JvmStatic fun c13(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|13"); return sb.toString() }
    @JvmStatic fun c14(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|14"); return sb.toString() }
    @JvmStatic fun c15(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|15"); return sb.toString() }
    @JvmStatic fun c16(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|16"); return sb.toString() }
    @JvmStatic fun c17(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|17"); return sb.toString() }
    @JvmStatic fun c18(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|18"); return sb.toString() }
    @JvmStatic fun c19(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|19"); return sb.toString() }
    @JvmStatic fun c20(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|20"); return sb.toString() }
    @JvmStatic fun c21(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|21"); return sb.toString() }
    @JvmStatic fun c22(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|22"); return sb.toString() }
    @JvmStatic fun c23(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|23"); return sb.toString() }
    @JvmStatic fun c24(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|24"); return sb.toString() }
    @JvmStatic fun c25(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|25"); return sb.toString() }
    @JvmStatic fun c26(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|26"); return sb.toString() }
    @JvmStatic fun c27(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|27"); return sb.toString() }
    @JvmStatic fun c28(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|28"); return sb.toString() }
    @JvmStatic fun c29(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|29"); return sb.toString() }
    @JvmStatic fun c30(x: String): String { val sb = StringBuilder(); sb.append("C:"); sb.append(x); sb.append("|30"); return sb.toString() }
}

object Main {
    // Entry point references every caller across all three groups so
    // R8 cannot tree-shake them. Each line builds the output
    // incrementally to defeat constant-folding into a single fold pass.
    @JvmStatic
    fun main(args: Array<String>) {
        val out = StringBuilder()
        val n = args.size
        out.append(GroupAInt.a01(n)); out.append(GroupAInt.a02(n + 1)); out.append(GroupAInt.a03(n + 2))
        out.append(GroupAInt.a04(n)); out.append(GroupAInt.a05(n + 1)); out.append(GroupAInt.a06(n + 2))
        out.append(GroupAInt.a07(n)); out.append(GroupAInt.a08(n + 1)); out.append(GroupAInt.a09(n + 2))
        out.append(GroupAInt.a10(n)); out.append(GroupAInt.a11(n + 1)); out.append(GroupAInt.a12(n + 2))
        out.append(GroupAInt.a13(n)); out.append(GroupAInt.a14(n + 1)); out.append(GroupAInt.a15(n + 2))
        out.append(GroupAInt.a16(n)); out.append(GroupAInt.a17(n + 1)); out.append(GroupAInt.a18(n + 2))
        out.append(GroupAInt.a19(n)); out.append(GroupAInt.a20(n + 1)); out.append(GroupAInt.a21(n + 2))
        out.append(GroupAInt.a22(n)); out.append(GroupAInt.a23(n + 1)); out.append(GroupAInt.a24(n + 2))
        out.append(GroupAInt.a25(n)); out.append(GroupAInt.a26(n + 1)); out.append(GroupAInt.a27(n + 2))
        out.append(GroupAInt.a28(n)); out.append(GroupAInt.a29(n + 1)); out.append(GroupAInt.a30(n + 2))

        val ln = n.toLong()
        out.append(GroupBLong.b01(ln)); out.append(GroupBLong.b02(ln + 1L)); out.append(GroupBLong.b03(ln + 2L))
        out.append(GroupBLong.b04(ln)); out.append(GroupBLong.b05(ln + 1L)); out.append(GroupBLong.b06(ln + 2L))
        out.append(GroupBLong.b07(ln)); out.append(GroupBLong.b08(ln + 1L)); out.append(GroupBLong.b09(ln + 2L))
        out.append(GroupBLong.b10(ln)); out.append(GroupBLong.b11(ln + 1L)); out.append(GroupBLong.b12(ln + 2L))
        out.append(GroupBLong.b13(ln)); out.append(GroupBLong.b14(ln + 1L)); out.append(GroupBLong.b15(ln + 2L))
        out.append(GroupBLong.b16(ln)); out.append(GroupBLong.b17(ln + 1L)); out.append(GroupBLong.b18(ln + 2L))
        out.append(GroupBLong.b19(ln)); out.append(GroupBLong.b20(ln + 1L)); out.append(GroupBLong.b21(ln + 2L))
        out.append(GroupBLong.b22(ln)); out.append(GroupBLong.b23(ln + 1L)); out.append(GroupBLong.b24(ln + 2L))
        out.append(GroupBLong.b25(ln)); out.append(GroupBLong.b26(ln + 1L)); out.append(GroupBLong.b27(ln + 2L))
        out.append(GroupBLong.b28(ln)); out.append(GroupBLong.b29(ln + 1L)); out.append(GroupBLong.b30(ln + 2L))

        val s = args.firstOrNull() ?: ""
        out.append(GroupCStr.c01(s)); out.append(GroupCStr.c02(s)); out.append(GroupCStr.c03(s))
        out.append(GroupCStr.c04(s)); out.append(GroupCStr.c05(s)); out.append(GroupCStr.c06(s))
        out.append(GroupCStr.c07(s)); out.append(GroupCStr.c08(s)); out.append(GroupCStr.c09(s))
        out.append(GroupCStr.c10(s)); out.append(GroupCStr.c11(s)); out.append(GroupCStr.c12(s))
        out.append(GroupCStr.c13(s)); out.append(GroupCStr.c14(s)); out.append(GroupCStr.c15(s))
        out.append(GroupCStr.c16(s)); out.append(GroupCStr.c17(s)); out.append(GroupCStr.c18(s))
        out.append(GroupCStr.c19(s)); out.append(GroupCStr.c20(s)); out.append(GroupCStr.c21(s))
        out.append(GroupCStr.c22(s)); out.append(GroupCStr.c23(s)); out.append(GroupCStr.c24(s))
        out.append(GroupCStr.c25(s)); out.append(GroupCStr.c26(s)); out.append(GroupCStr.c27(s))
        out.append(GroupCStr.c28(s)); out.append(GroupCStr.c29(s)); out.append(GroupCStr.c30(s))

        println(out.toString())
    }
}
