// adapted from R8 test src/test/examples/classmerging/ConflictingInterfaceSignaturesTest.java (Apache-2)
// Exercises a class implementing two interfaces that declare the same
// method signature `void foo()`. JVMS resolves both interface dispatches
// to the single concrete `InterfaceImpl.foo`; DEX `invoke-interface` carries
// the receiver-type-as-declared-by-the-callsite, so the two call sites
// (`a.foo()` with a:A vs `b.foo()` with b:B) reference different `method_id`s
// even though they target the same body. Decompile must keep both
// interface declarations and the `implements A, B` clause.
public class DualInterfaceSameSig {
    public static void main(String[] args) {
        A a = new InterfaceImpl();
        a.foo();

        B b = new InterfaceImpl();
        b.foo();
    }

    public interface A {
        void foo();
    }

    public interface B {
        void foo();
    }

    public static final class InterfaceImpl implements A, B {
        @Override
        public void foo() {
            System.out.println("In foo on InterfaceImpl");
        }
    }
}
