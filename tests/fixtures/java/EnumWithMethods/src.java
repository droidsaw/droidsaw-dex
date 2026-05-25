// Covers a Java 5+ enum with per-constant method bodies. Each constant
// (`ADD { int apply(...) {...} }`) compiles to an anonymous subclass of
// the enum type stored in the enum's static constant array. Decompile
// must recognise the `$values` init pattern and the anonymous subclass
// chain and restore the per-constant body form.
public class EnumWithMethods {
    enum Op {
        ADD {
            @Override
            int apply(int a, int b) { return a + b; }
        },
        SUB {
            @Override
            int apply(int a, int b) { return a - b; }
        };

        abstract int apply(int a, int b);
    }

    public static void main(String[] args) {
        System.out.println(Op.ADD.apply(3, 4));
        System.out.println(Op.SUB.apply(10, 2));
    }
}
