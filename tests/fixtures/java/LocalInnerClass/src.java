// Covers a method-local inner class. javac compiles to a named class
// `LocalInnerClass$1Adder` with synthetic fields for each captured
// effectively-final local (here `base`). Unlike an anonymous class the
// local inner has a declared name, so decompile must restore a class
// declaration inside the enclosing method body rather than a top-level
// or anonymous form.
public class LocalInnerClass {
    public static void main(String[] args) {
        final int base = 100;
        class Adder {
            int add(int x) {
                return base + x;
            }
        }
        Adder a = new Adder();
        System.out.println(a.add(5));
        System.out.println(a.add(10));
    }
}
