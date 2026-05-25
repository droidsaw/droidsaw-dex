// AndroidX-style library-helper rename fixture.
//
// Package `androidx.testlib` mimics the namespace shape of a real
// AndroidX module (e.g. androidx.lifecycle, androidx.appcompat). The
// class `LifecycleHelpers` hosts ~25 small @JvmStatic helper methods.
// Each helper:
//
//   - Is `public static` (R8 emits these — satisfies recogniser I4).
//   - Is straight-line, no branches (satisfies I6).
//   - Has body length in the 3-10 invocation range (satisfies I7's
//     3-99-bytes window once compiled to DEX).
//   - Takes a single primitive or String parameter (satisfies I8's
//     arity <= 5 and I13's param_count >= 1).
//
// The methods are intentionally STRUCTURALLY DIFFERENT from each
// other so R8's outliner does NOT extract a shared body. We want
// minification + rename only, not outline-synthesis. Each helper
// uses a different sequence of stdlib calls + constants — different
// prefixes, different append() overloads, occasional Math/Integer
// calls, distinct return shapes — so the outliner's
// repeated-body-across-20+-callers predicate cannot match.
//
// Main calls every helper once. Without these call sites R8 would
// dead-code-eliminate the helpers (defeating the FP demo) because
// proguard-rules.pro intentionally does NOT keep the helper class.
// Only the Main entry is kept; LifecycleHelpers stays only because
// Main references it, and its methods stay only because Main calls
// them. Both class + methods are then minified to short names.
//
// Why this is a recogniser FP demo:
//
// Each individual helper body, after R8 minification, satisfies I4
// through I13. The recogniser fires structurally on each renamed
// `androidx.testlib.X.a`, `.b`, `.c`, ... even though the mapping
// contains zero `com.android.tools.r8.outline` annotations on these
// tuples. The mapping records the renames but flags them as ORDINARY
// minified application code, not as R8-synthesised outlines. The
// recogniser without an AndroidX family-FP entry has no way to tell
// these apart from a genuine outline helper.

package androidx.testlib

object LifecycleHelpers {
    @JvmStatic fun dispatchObserve(x: Int): String {
        val sb = StringBuilder(); sb.append("obs:"); sb.append(x); return sb.toString()
    }

    @JvmStatic fun stateChange(x: Int): String {
        val sb = StringBuilder(); sb.append("state="); sb.append(x); sb.append('/'); return sb.toString()
    }

    @JvmStatic fun fireEvent(x: Int): String {
        val sb = StringBuilder(); sb.append("evt#"); sb.append(x + 1); return sb.toString()
    }

    @JvmStatic fun lifecycleTag(x: Int): String {
        return "[lc:" + x.toString() + "]"
    }

    @JvmStatic fun observerKey(x: Long): String {
        val sb = StringBuilder(); sb.append("ok:"); sb.append(x); sb.append("|L"); return sb.toString()
    }

    @JvmStatic fun handlerName(x: Long): String {
        return "handler-" + java.lang.Long.toHexString(x)
    }

    @JvmStatic fun resumeMarker(x: Int): String {
        val sb = StringBuilder(); sb.append("RESUMED["); sb.append(x); sb.append(']'); return sb.toString()
    }

    @JvmStatic fun pauseMarker(x: Int): String {
        val sb = StringBuilder(); sb.append("PAUSED@"); sb.append(x * 2); return sb.toString()
    }

    @JvmStatic fun startMarker(x: Int): String {
        return "STARTED" + (x - 1).toString()
    }

    @JvmStatic fun stopMarker(x: Int): String {
        val sb = StringBuilder(2); sb.append("S<"); sb.append(x); sb.append('>'); return sb.toString()
    }

    @JvmStatic fun viewTag(x: String): String {
        val sb = StringBuilder(); sb.append("vt:"); sb.append(x); sb.append("//"); return sb.toString()
    }

    @JvmStatic fun viewId(x: String): String {
        return x + "#id"
    }

    @JvmStatic fun viewKey(x: String): String {
        val sb = StringBuilder(); sb.append('K'); sb.append('='); sb.append(x); return sb.toString()
    }

    @JvmStatic fun fragmentTag(x: String): String {
        val sb = StringBuilder(); sb.append("frag:"); sb.append(x.length); sb.append(':'); sb.append(x); return sb.toString()
    }

    @JvmStatic fun resourceName(x: Int): String {
        return "res/" + Integer.toHexString(x)
    }

    @JvmStatic fun safeAdd(a: Int): Int {
        return Math.addExact(a, 7)
    }

    @JvmStatic fun safeMul(a: Int): Int {
        return Math.multiplyExact(a, 3)
    }

    @JvmStatic fun absClamp(a: Int): Int {
        return Math.max(Math.abs(a), 1)
    }

    @JvmStatic fun bitWidth(a: Int): Int {
        return 32 - Integer.numberOfLeadingZeros(a)
    }

    @JvmStatic fun parity(a: Long): Int {
        return java.lang.Long.bitCount(a) and 1
    }

    @JvmStatic fun upperTrim(x: String): String {
        return x.trim().uppercase()
    }

    @JvmStatic fun lowerTrim(x: String): String {
        return x.trim().lowercase()
    }

    @JvmStatic fun escapeQuote(x: String): String {
        val sb = StringBuilder(); sb.append('"'); sb.append(x); sb.append('"'); return sb.toString()
    }

    @JvmStatic fun bracketKey(x: String): String {
        return "[" + x + "]"
    }

    @JvmStatic fun versionTag(x: Int): String {
        val sb = StringBuilder(); sb.append("v"); sb.append(x / 10); sb.append('.'); sb.append(x % 10); return sb.toString()
    }
}

object Main {
    // Reference every helper exactly once so R8 keeps them but the
    // outliner's >= 20 distinct callers predicate cannot match
    // (each body is unique, no body is repeated across callers).
    @JvmStatic
    fun main(args: Array<String>) {
        val n = args.size
        val ln = n.toLong()
        val s = args.firstOrNull() ?: ""
        val out = StringBuilder()

        out.append(LifecycleHelpers.dispatchObserve(n))
        out.append(LifecycleHelpers.stateChange(n))
        out.append(LifecycleHelpers.fireEvent(n))
        out.append(LifecycleHelpers.lifecycleTag(n))
        out.append(LifecycleHelpers.observerKey(ln))
        out.append(LifecycleHelpers.handlerName(ln))
        out.append(LifecycleHelpers.resumeMarker(n))
        out.append(LifecycleHelpers.pauseMarker(n))
        out.append(LifecycleHelpers.startMarker(n))
        out.append(LifecycleHelpers.stopMarker(n))
        out.append(LifecycleHelpers.viewTag(s))
        out.append(LifecycleHelpers.viewId(s))
        out.append(LifecycleHelpers.viewKey(s))
        out.append(LifecycleHelpers.fragmentTag(s))
        out.append(LifecycleHelpers.resourceName(n))
        out.append(LifecycleHelpers.safeAdd(n))
        out.append(LifecycleHelpers.safeMul(n))
        out.append(LifecycleHelpers.absClamp(n))
        out.append(LifecycleHelpers.bitWidth(n))
        out.append(LifecycleHelpers.parity(ln))
        out.append(LifecycleHelpers.upperTrim(s))
        out.append(LifecycleHelpers.lowerTrim(s))
        out.append(LifecycleHelpers.escapeQuote(s))
        out.append(LifecycleHelpers.bracketKey(s))
        out.append(LifecycleHelpers.versionTag(n))

        println(out.toString())
    }
}
