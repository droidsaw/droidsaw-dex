// Aspirational Java that the Wave 2A `dex-r8-signatures` recogniser
// emits when decompiling the R8-values-cached output. The cached field
// is rewritten back to a `Color.values()` call at each use; the field
// is suppressed from emit (it's a synthetic).
public class Simple {
    public enum Color { RED, GREEN, BLUE }

    /* @droidsaw R8Origin(EnumValuesCached, helper=$cached-values) */
    public static int countRed() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.RED) c++;
        }
        return c;
    }

    /* @droidsaw R8Origin(EnumValuesCached, helper=$cached-values) */
    public static int countGreen() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.GREEN) c++;
        }
        return c;
    }

    /* @droidsaw R8Origin(EnumValuesCached, helper=$cached-values) */
    public static int countBlue() {
        int c = 0;
        for (Color v : Color.values()) {
            if (v == Color.BLUE) c++;
        }
        return c;
    }
}
