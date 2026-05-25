// A generic static method plus a call site. Generics are erased in
// DEX bytecode, so the decompiler currently emits the erased signature;
// runtime semantics are preserved regardless.
import java.util.ArrayList;
import java.util.List;

public class GenericMethod {
    static <T> T first(List<T> xs, T fallback) {
        if (xs == null || xs.isEmpty()) {
            return fallback;
        }
        return xs.get(0);
    }

    public static void main(String[] args) {
        List<String> ss = new ArrayList<String>();
        ss.add("hello");
        ss.add("world");
        System.out.println(first(ss, "nope"));

        List<Integer> is = new ArrayList<Integer>();
        System.out.println(first(is, -1));
    }
}
