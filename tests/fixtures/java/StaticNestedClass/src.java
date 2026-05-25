// Covers a static nested class. Dalvik emits Outer$Inner as a separate
// class_def referencing the outer via `enclosing_class` attribute; there
// is no synthetic this$0 field because `static` breaks the enclosing-
// instance link. Decompile must restore the `static class Inner` form
// (not inner-instance form) and avoid re-emitting a nonexistent outer-
// reference field.
public class StaticNestedClass {
    static class Counter {
        private int n;

        int incrAndGet() {
            return ++n;
        }
    }

    public static void main(String[] args) {
        Counter c = new Counter();
        System.out.println(c.incrAndGet());
        System.out.println(c.incrAndGet());
        System.out.println(c.incrAndGet());
    }
}
