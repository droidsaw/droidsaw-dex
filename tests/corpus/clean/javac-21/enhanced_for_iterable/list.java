// javac-21 corpus seed: enhanced-for over Iterable.
// Lowers to `Iterator it = xs.iterator(); while (it.hasNext()) { Integer x = it.next(); ... }`.
import java.util.List;
class T {
    static int sum(List<Integer> xs) {
        int s = 0;
        for (Integer x : xs) {
            s += x;
        }
        return s;
    }
}
