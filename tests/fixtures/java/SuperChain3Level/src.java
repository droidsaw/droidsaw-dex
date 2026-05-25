// adapted from R8 test src/test/examples/classmerging/RewritePinnedMethodTest.java (Apache-2)
// Exercises a 3-level `super.m()` invocation chain. C extends B extends A,
// each overrides `m()` and chains via `super.m()`. DEX `invoke-super`
// resolves on the static class context, not the dynamic receiver, so each
// link in the chain becomes a distinct invoke-super site referencing the
// immediate parent. The decompiler must preserve the chain exactly —
// rewriting `super.m()` to `this.m()` would re-enter `C.m()` and infinite-loop.
public class SuperChain3Level {
    public static void main(String[] args) {
        new C().m();
    }

    public static class A {
        public void m() {
            System.out.println("In A.m");
        }
    }

    public static class B extends A {
        @Override
        public void m() {
            System.out.println("In B.m");
            super.m();
        }
    }

    public static class C extends B {
        @Override
        public void m() {
            System.out.println("In C.m");
            super.m();
        }
    }
}
