// adapted from R8 test src/test/examples/classmerging/SyntheticBridgeSignaturesTest.java (Apache-2)
// Exercises symmetric overloads between two peer (non-related) classes:
// `A.m(B)` and `B.m(A)`. ASub extends A and BSub extends B, so the dynamic
// types of the receivers (ASub, BSub) are subclasses but the param types
// (B, A) are the static base classes. Tests that the decompiler resolves
// the `method_id` operand of each invoke against the declared param type,
// not the dynamic argument type — confusing the two would cause infinite
// recursion (a.m(b) → calls B.m(A) → calls A.m(B) → ...).
public class OverloadsPeerClasses {
    public static void main(String[] args) {
        ASub a = new ASub();
        BSub b = new BSub();
        a.m(b);
        b.m(a);
    }

    private static class A {
        public void m(B object) {
            System.out.println("In A.m()");
        }
    }

    private static class ASub extends A {}

    private static class B {
        public void m(A object) {
            System.out.println("In B.m()");
        }
    }

    private static class BSub extends B {}
}
