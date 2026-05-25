// Hand-written Kotlin source that mimics the SHAPE of flutter_embedding's
// library helper classes (e.g. io.flutter.plugin.platform.PlatformPlugin
// + io.flutter.plugin.common.MethodChannel helper sites): an outer
// container with ~24 short static methods whose bodies are SIMILAR
// (single Int / String param, straight-line, a handful of invokes +
// a return) but NOT IDENTICAL. The not-identical part is load-bearing:
//
//   * If the bodies were identical to each other, R8's outliner would
//     extract them into a synthetic helper class and the recogniser
//     would correctly fire on a TRUE positive (this is the
//     block_outlining fixture next door).
//   * If the bodies merely *resemble* one another — same arity, same
//     return type, similar invoke pattern — R8's MINIFIER still
//     renames every class+method to single-letter names, but the
//     OUTLINER does NOT fire because the bodies fail R8's
//     identical-instruction-sequence gate. The result is short-named
//     classes that look like outlines to a structural recogniser, but
//     whose mapping carries no `com.android.tools.r8.outline`
//     annotation.
//
// That distinction is the empirical anchor for the `io.flutter`
// known-FP family entry: Flutter's keep rules (-dontwarn +
// allowshrinking,allowobfuscation; see
// packages/flutter_tools/gradle/flutter_proguard_rules.pro) explicitly allow R8
// to rename flutter_embedding's helper classes, which the upstream
// engine ships as a regular Maven `api` dependency that R8 sees as
// program input (engine/src/flutter/shell/platform/android/BUILD.gn
// :213-215, :858-915). Renamed library code is not synthetic; it just
// looks synthetic to a structural recogniser.
//
// Source shape mirrors what was observed in real R8-shrunk Flutter
// APKs surveyed by the dex-r8-outliner-fp-family-survey work:
//
//   * Static methods (I4: ACC_PUBLIC | ACC_STATIC after R8 emit).
//   * 1-2 typed params, arity well under 5 (I8).
//   * Straight-line body, no branches (I6).
//   * Body size between 3 and 99 bytes (I7).
//   * >= 20 distinct callers across Main (I9, I13).
//   * Param count >= 1 (I13).
//
// Bodies must NOT collapse to identical instruction sequences. We
// achieve that by varying: the constant prefix appended, the operand
// position of the param, the suffix shape, and whether the param is
// stringified directly or formatted first.

package fox.droidsaw.r8fixture.libstub

object MyLibraryHelpers {
    // Variants on "build a tagged string from an Int". The append
    // sequence varies in length and ordering, so R8 cannot extract a
    // shared body.

    @JvmStatic fun fmt(x: Int): String {
        val sb = StringBuilder()
        sb.append("fmt:")
        sb.append(x)
        return sb.toString()
    }

    @JvmStatic fun pad(x: Int): String {
        val sb = StringBuilder()
        sb.append("[")
        sb.append(x)
        sb.append("]")
        return sb.toString()
    }

    @JvmStatic fun tag(x: Int): String {
        val sb = StringBuilder()
        sb.append("tag<")
        sb.append(x)
        sb.append(">")
        return sb.toString()
    }

    @JvmStatic fun hex(x: Int): String {
        val sb = StringBuilder()
        sb.append("0x")
        sb.append(Integer.toHexString(x))
        return sb.toString()
    }

    @JvmStatic fun dec(x: Int): String {
        val sb = StringBuilder()
        sb.append(x)
        sb.append("d")
        return sb.toString()
    }

    @JvmStatic fun oct(x: Int): String {
        val sb = StringBuilder()
        sb.append("0o")
        sb.append(Integer.toOctalString(x))
        return sb.toString()
    }

    @JvmStatic fun bin(x: Int): String {
        val sb = StringBuilder()
        sb.append("0b")
        sb.append(Integer.toBinaryString(x))
        return sb.toString()
    }

    @JvmStatic fun neg(x: Int): String {
        val sb = StringBuilder()
        sb.append("-")
        sb.append(x)
        return sb.toString()
    }

    @JvmStatic fun pos(x: Int): String {
        val sb = StringBuilder()
        sb.append("+")
        sb.append(x)
        return sb.toString()
    }

    @JvmStatic fun bra(x: Int): String {
        val sb = StringBuilder()
        sb.append("(")
        sb.append(x)
        sb.append(")")
        return sb.toString()
    }

    @JvmStatic fun cur(x: Int): String {
        val sb = StringBuilder()
        sb.append("{")
        sb.append(x)
        sb.append("}")
        return sb.toString()
    }

    @JvmStatic fun pct(x: Int): String {
        val sb = StringBuilder()
        sb.append(x)
        sb.append("%")
        return sb.toString()
    }

    // String-param variants. The methods do the same KIND of work
    // (formatting an inbound string into a tag) but with enough
    // structural variation that the outliner cannot extract a shared
    // body.

    @JvmStatic fun upper(s: String): String {
        val sb = StringBuilder()
        sb.append("U:")
        sb.append(s.uppercase())
        return sb.toString()
    }

    @JvmStatic fun lower(s: String): String {
        val sb = StringBuilder()
        sb.append("l:")
        sb.append(s.lowercase())
        return sb.toString()
    }

    @JvmStatic fun rev(s: String): String {
        val sb = StringBuilder()
        sb.append("r:")
        sb.append(s.reversed())
        return sb.toString()
    }

    @JvmStatic fun pre(s: String): String {
        val sb = StringBuilder()
        sb.append("pre/")
        sb.append(s)
        return sb.toString()
    }

    @JvmStatic fun post(s: String): String {
        val sb = StringBuilder()
        sb.append(s)
        sb.append("/post")
        return sb.toString()
    }

    @JvmStatic fun wrap(s: String): String {
        val sb = StringBuilder()
        sb.append("<<")
        sb.append(s)
        sb.append(">>")
        return sb.toString()
    }

    @JvmStatic fun trim(s: String): String {
        val sb = StringBuilder()
        sb.append("t:")
        sb.append(s.trim())
        return sb.toString()
    }

    @JvmStatic fun lenOf(s: String): String {
        val sb = StringBuilder()
        sb.append("len=")
        sb.append(s.length)
        return sb.toString()
    }

    @JvmStatic fun head(s: String): String {
        val sb = StringBuilder()
        sb.append("h:")
        sb.append(if (s.isEmpty()) "" else s.substring(0, 1))
        return sb.toString()
    }

    @JvmStatic fun tail(s: String): String {
        val sb = StringBuilder()
        sb.append("t:")
        sb.append(if (s.isEmpty()) "" else s.substring(s.length - 1))
        return sb.toString()
    }

    @JvmStatic fun double(s: String): String {
        val sb = StringBuilder()
        sb.append(s)
        sb.append(s)
        return sb.toString()
    }

    @JvmStatic fun bang(s: String): String {
        val sb = StringBuilder()
        sb.append(s)
        sb.append("!")
        return sb.toString()
    }
}

object Main {
    // Every helper is invoked from at least two distinct call sites
    // to satisfy R8's reachability gate (helpers with a single caller
    // get inlined). The "many callers" structural shape is also part
    // of what makes the recogniser fire on R8-renamed library helpers
    // in the wild — flutter_embedding's PlatformPlugin / MethodChannel
    // helpers similarly have a dense fan-in from generated platform
    // glue.
    @JvmStatic
    fun main(args: Array<String>) {
        val n = args.size
        val s = args.firstOrNull() ?: "x"
        val out = StringBuilder()

        // First pass: every helper once.
        out.append(MyLibraryHelpers.fmt(n))
        out.append(MyLibraryHelpers.pad(n + 1))
        out.append(MyLibraryHelpers.tag(n + 2))
        out.append(MyLibraryHelpers.hex(n + 3))
        out.append(MyLibraryHelpers.dec(n + 4))
        out.append(MyLibraryHelpers.oct(n + 5))
        out.append(MyLibraryHelpers.bin(n + 6))
        out.append(MyLibraryHelpers.neg(n + 7))
        out.append(MyLibraryHelpers.pos(n + 8))
        out.append(MyLibraryHelpers.bra(n + 9))
        out.append(MyLibraryHelpers.cur(n + 10))
        out.append(MyLibraryHelpers.pct(n + 11))
        out.append(MyLibraryHelpers.upper(s))
        out.append(MyLibraryHelpers.lower(s))
        out.append(MyLibraryHelpers.rev(s))
        out.append(MyLibraryHelpers.pre(s))
        out.append(MyLibraryHelpers.post(s))
        out.append(MyLibraryHelpers.wrap(s))
        out.append(MyLibraryHelpers.trim(s))
        out.append(MyLibraryHelpers.lenOf(s))
        out.append(MyLibraryHelpers.head(s))
        out.append(MyLibraryHelpers.tail(s))
        out.append(MyLibraryHelpers.double(s))
        out.append(MyLibraryHelpers.bang(s))

        // Second pass: same helpers, different operands. Guarantees
        // every helper has >= 2 distinct call sites (i.e. R8 will not
        // inline them as single-caller dead weight).
        out.append(MyLibraryHelpers.fmt(n + 100))
        out.append(MyLibraryHelpers.pad(n + 101))
        out.append(MyLibraryHelpers.tag(n + 102))
        out.append(MyLibraryHelpers.hex(n + 103))
        out.append(MyLibraryHelpers.dec(n + 104))
        out.append(MyLibraryHelpers.oct(n + 105))
        out.append(MyLibraryHelpers.bin(n + 106))
        out.append(MyLibraryHelpers.neg(n + 107))
        out.append(MyLibraryHelpers.pos(n + 108))
        out.append(MyLibraryHelpers.bra(n + 109))
        out.append(MyLibraryHelpers.cur(n + 110))
        out.append(MyLibraryHelpers.pct(n + 111))
        out.append(MyLibraryHelpers.upper(s + "x"))
        out.append(MyLibraryHelpers.lower(s + "y"))
        out.append(MyLibraryHelpers.rev(s + "z"))
        out.append(MyLibraryHelpers.pre(s + "1"))
        out.append(MyLibraryHelpers.post(s + "2"))
        out.append(MyLibraryHelpers.wrap(s + "3"))
        out.append(MyLibraryHelpers.trim(s + "4"))
        out.append(MyLibraryHelpers.lenOf(s + "5"))
        out.append(MyLibraryHelpers.head(s + "6"))
        out.append(MyLibraryHelpers.tail(s + "7"))
        out.append(MyLibraryHelpers.double(s + "8"))
        out.append(MyLibraryHelpers.bang(s + "9"))

        println(out.toString())
    }
}
