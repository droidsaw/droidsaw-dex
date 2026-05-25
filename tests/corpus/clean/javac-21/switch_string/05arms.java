// javac-21 corpus seed: 5-arm String switch (mid-size).
class T {
    static String f(String k) {
        switch (k) {
            case "a": return "A";
            case "b": return "B";
            case "c": return "C";
            case "d": return "D";
            case "e": return "E";
        }
        return "?";
    }
}
