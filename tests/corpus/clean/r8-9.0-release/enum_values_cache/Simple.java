// `MyEnum.values()` is called repeatedly. R8 lifts the repeated
// `values()` call into a single static field initialised once at class
// load, and rewrites each use to a field read.
//
// Recogniser intent (Wave 2A `dex-r8-signatures`): detect a static
// field whose type matches the enum's `[]` array AND whose only
// writer is `<clinit>` calling `Enum.values()`. Rewrite reads of that
// field back to `MyEnum.values()` calls in the decompiled output.
public class Simple {
    public enum Color {
        RED, GREEN, BLUE
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
}
