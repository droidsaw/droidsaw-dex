// javac-21 corpus seed: 5-arm sparse-int switch.
// javac chooses lookupswitch (binary-search) when arms are sparsely
// distributed in the int range.
class T {
    static String f(int x) {
        switch (x) {
            case 1:         return "a";
            case 100:       return "b";
            case 10000:     return "c";
            case 1000000:   return "d";
            case 100000000: return "e";
        }
        return "?";
    }
}
