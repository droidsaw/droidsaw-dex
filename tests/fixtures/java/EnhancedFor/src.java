// Covers enhanced-for over an Iterable (sibling `ForEach.java` under
// the roundtrip test set already covers arrays). javac compiles this
// to an explicit `Iterator it = list.iterator(); while (it.hasNext())
// { X x = it.next(); ... }` sequence. Decompile must recognise the
// pattern and restore `for (X x : c)` rather than emitting the raw
// Iterator calls.
import java.util.Arrays;
import java.util.List;

public class EnhancedFor {
    static int sumList(List<Integer> xs) {
        int sum = 0;
        for (Integer x : xs) {
            sum += x;
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(sumList(Arrays.asList(1, 2, 3, 4)));
        System.out.println(sumList(Arrays.asList(10, 20, 30)));
    }
}
