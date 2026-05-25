// javac-21 corpus seed: 5-arm dense-int switch.
// javac chooses tableswitch (vs lookupswitch) when arms are densely packed.
class T {
    static String f(int x) {
        switch (x) {
            case 1: return "a";
            case 2: return "b";
            case 3: return "c";
            case 4: return "d";
            case 5: return "e";
        }
        return "?";
    }
}
