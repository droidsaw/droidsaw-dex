// javac-21 corpus seed: 2-arm String switch.
// Lowers to outer tableswitch(String.hashCode()) + per-arm equals-check +
// inner tableswitch(tag).
class T {
    static String f(String k) {
        switch (k) {
            case "a": return "A";
            case "b": return "B";
        }
        return "?";
    }
}
