// adapted from R8 test src/test/examplesAndroidN/interfacemethods/StaticInterfaceMethods.java
// + I1.java (Apache-2). Two-file source inlined into a single fixture by
// hoisting `interface I1` to a nested type inside the test class.
//
// Exercises a `static` method on an interface, a Java 8 surface that
// requires d8 desugaring on `--min-api < 24`. With `d8_min_api = 19`
// (set on this fixture's manifest entry), d8 lifts `I1.s1()` into a
// synthetic companion class (typically named `I1$-CC`) and rewrites the
// call site `I1.s1()` to invoke the companion. Decompile must either
// (a) recognise the desugar pattern and restore the original `static`
// declaration on the interface, or (b) emit the companion-class form
// verbatim — the manifest classification depends on which.
public class StaticInterfaceMethod {
    public interface I1 {
        static void s1() {
            System.out.println("s1");
        }
    }

    public static void main(String[] args) {
        I1.s1();
    }
}
