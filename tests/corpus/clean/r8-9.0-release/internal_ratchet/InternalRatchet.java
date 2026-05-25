// Internal-build ratchet fixture for the R8 oracle ratchet's
// mapping-paired validation.
//
// Goal: produce at least one `com.android.tools.r8.synthesized`
// class in the resulting mapping.txt so the ratchet's
// FP-cross-reference logic has something non-trivial to validate
// against. The Wave 1B per-construct fixtures exercise one R8
// transformation each; this fixture is a mixed-shape ratchet input
// that exercises the END-TO-END pipeline: corpus walker → R8
// produces (classes.dex, mapping.txt) → ratchet reads both →
// asserts every recogniser marker corresponds to a synthesized
// class.
//
// Mix of shapes:
//
// - **Enum + values()** — R8 9.0's EnumUnboxingSharedUtility lands
//   here. This is the most reliable way to get a
//   `com.android.tools.r8.synthesized` annotation in the mapping
//   from a small Java source.
//
// - **Repeated-arithmetic drivers** — six entry points that share
//   compute kernels. R8 may inline these (Wave 1B observation) or
//   outline (less common at small fixture size). Either way the
//   mapping records what happened; the ratchet uses that record.
//
// - **Control samples** — short and branchy methods R8 should
//   leave alone. Negative cases: a recogniser marker on these
//   would be an FP.
public class InternalRatchet {

    // Enum + values() iteration. R8's enum-unboxing optimisation
    // creates a synthetic helper class
    // (`InternalRatchet$Color$EnumUnboxingSharedUtility -> <obf>`)
    // annotated `com.android.tools.r8.synthesized`. That's the
    // anchor class the ratchet's FP-check exercises against.
    public enum Color {
        RED, GREEN, BLUE, ALPHA, WHITE, BLACK
    }

    public static int countRed() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.RED) c++;
        }
        return c;
    }

    public static int countGreen() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.GREEN) c++;
        }
        return c;
    }

    public static int countBlue() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.BLUE) c++;
        }
        return c;
    }

    public static int countAlpha() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.ALPHA) c++;
        }
        return c;
    }

    public static int countWhite() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.WHITE) c++;
        }
        return c;
    }

    public static int countBlack() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.BLACK) c++;
        }
        return c;
    }

    // Repeated-arithmetic drivers. R8 may inline these helpers
    // (small leaf shape) or outline a shared block; either way
    // the mapping records the outcome.

    public static int driverA(int x, int y) {
        return mixA(x, y, 17) + mixA(x + 1, y + 1, 17) + mixA(x + 2, y + 2, 17);
    }

    public static int driverB(int x, int y) {
        return mixB(x, y, 23) + mixB(x + 1, y + 1, 23) + mixB(x + 2, y + 2, 23);
    }

    public static int driverC(int x, int y) {
        return mixC(x, y, 31) + mixC(x + 1, y + 1, 31) + mixC(x + 2, y + 2, 31);
    }

    private static int mixA(int x, int y, int salt) {
        int r = x * salt;
        r = r + y * (salt + 6);
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    private static int mixB(int x, int y, int salt) {
        int r = x * salt;
        r = r + y * (salt + 6);
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    private static int mixC(int x, int y, int salt) {
        int r = x * salt;
        r = r + y * (salt + 6);
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    // Control samples — methods R8 should leave alone.

    public static int controlSimple(int x) {
        return x + 1;
    }

    public static int controlBranchy(int x) {
        if (x < 0) return -1;
        if (x < 10) return x;
        if (x < 100) return x * 2;
        return x * 4;
    }
}
