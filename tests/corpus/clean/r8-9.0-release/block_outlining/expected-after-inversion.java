// Aspirational Java that the Wave 2A `dex-r8-signatures` recogniser
// should produce when decompiling the R8-outlined output of
// `RepeatedPattern.java`. The recogniser detects the trampoline shape
// (single invoke-static + return) and inlines the synthetic helper's
// body at the call site, tagging the inversion with `R8Origin`.
//
// Note: original method names cannot be recovered without a
// `--mapping-file`; the recogniser preserves the d8/R8 names but
// surfaces the body shape that was outlined.
public class RepeatedPattern {
    /* @droidsaw R8Origin(BlockOutlined, helper=$synthetic) */
    public static int methodA(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    /* @droidsaw R8Origin(BlockOutlined, helper=$synthetic) */
    public static int methodB(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    /* @droidsaw R8Origin(BlockOutlined, helper=$synthetic) */
    public static int methodC(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    public static int driver(int x, int y) {
        return methodA(x, y) + methodB(x, y) + methodC(x, y);
    }
}
