// adapted from R8 test src/test/examples/classmerging/MethodCollisionTest.java (Apache-2)
// Exercises private methods with identical signatures across an
// inheritance chain. Private methods do NOT participate in dynamic
// dispatch — `A.m` and `B.m` are unrelated symbols even though
// declared in parent/child classes. `A.invokeM()` (public) calls
// `m()` and resolves at compile time to A.m, even when invoked on
// a B receiver. DEX `invoke-direct` (private) vs `invoke-virtual` is
// the discriminator. Separately, D extends C with public m()+super.m()
// — that DOES override.
public class PrivateMethodCollision {
    public static void main(String[] args) {
        B b = new B();
        b.invokeM();

        D d = new D();
        d.m();
    }

    public static class A {
        private void m() {
            System.out.println("A.m");
        }

        public void invokeM() {
            m();
        }
    }

    public static class B extends A {
        private void m() {
            System.out.println("B.m");
        }
    }

    public static class C {
        public void m() {
            System.out.println("C.m");
        }
    }

    public static class D extends C {
        @Override
        public void m() {
            System.out.println("D.m");
            super.m();
        }
    }
}
