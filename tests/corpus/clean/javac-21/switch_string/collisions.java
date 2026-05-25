// javac-21 corpus seed: hashCode-collision arms.
// "Aa" and "BB" have the same hashCode (2112) in Java; javac lowers
// colliding arms into an inner equals()-chain within the same outer switch
// arm. The recognizer must handle the bucket form correctly.
class T {
    static String f(String k) {
        switch (k) {
            case "Aa": return "x";
            case "BB": return "y";
            case "C":  return "z";
        }
        return "?";
    }
}
