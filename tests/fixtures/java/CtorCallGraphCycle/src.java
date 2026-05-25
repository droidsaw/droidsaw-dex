// adapted from R8 test src/test/examples/classmerging/CallGraphCycleTest.java (Apache-2)
// Exercises a constructor call graph with conditional branching + super().
// `B extends A`; B(..) calls super(..) which conditionally constructs
// another B(..). The outer `new B(true, false)` therefore drives the
// chain B(true)→A(true)→[new B(false)→A(false)·B(false)]→A(true)·B(true).
// Tests that the decompiler preserves the conditional inside `<init>`
// and the super() invocation order, neither of which can be reordered.
public class CtorCallGraphCycle {
    public static void main(String[] args) {
        new B(args.length == 0, args.length == 1);
    }

    public static class A {
        public A(boolean instantiateB, boolean alwaysFalse) {
            if (instantiateB) {
                new B(alwaysFalse, alwaysFalse);
            }
            System.out.println("A(" + instantiateB + ")");
        }
    }

    public static class B extends A {
        public B(boolean instantiateBinA, boolean alwaysFalse) {
            super(instantiateBinA, alwaysFalse);
            System.out.println("B(" + instantiateBinA + ")");
        }
    }
}
