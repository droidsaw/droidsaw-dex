// Exercises decompile_class on a class with no fields and no methods
// other than the implicit default constructor. The companion Holder
// ensures at least one observable side effect at runtime.
class Holder {
}

public class EmptyClass {
    public static void main(String[] args) {
        Holder h = new Holder();
        System.out.println(h == null ? "null" : "ok");
    }
}
