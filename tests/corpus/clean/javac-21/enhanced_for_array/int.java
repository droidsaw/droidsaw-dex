// javac-21 corpus seed: enhanced-for over int[].
// Distinct from Iterable form: javac lowers to indexed loop:
//   int[] $tmp = xs; int $len = $tmp.length;
//   for (int $i = 0; $i < $len; $i++) { int x = $tmp[$i]; ... }
class T {
    static int sum(int[] xs) {
        int s = 0;
        for (int x : xs) {
            s += x;
        }
        return s;
    }
}
