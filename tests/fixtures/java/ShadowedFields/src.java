// adapted from R8 test src/test/examples/classmerging/FieldCollisionTest.java (Apache-2)
// Exercises field shadowing: B extends A with both declaring a field
// named `obj` of compatible type. JVMS keeps both fields physically
// (no override for fields), distinguished by declaring class. The cast
// `((B) super.obj).message` in B.toString explicitly reads A's field
// via super, then casts the value back to B to project `.message`.
// Decompile must preserve the `super.obj` access (rewriting it to
// `obj` would silently read B's field instead, changing semantics).
public class ShadowedFields {
    private static final B SENTINEL_A = new B("A");
    private static final B SENTINEL_B = new B("B");

    public static void main(String[] args) {
        B obj = new B();
        System.out.println(obj.toString());
    }

    public static class A {
        protected final A obj = SENTINEL_A;
    }

    public static class B extends A {
        protected final String message;
        protected final B obj = SENTINEL_B;

        public B() {
            this(null);
        }

        public B(String message) {
            this.message = message;
        }

        @Override
        public String toString() {
            return obj.message + System.lineSeparator() + ((B) super.obj).message;
        }
    }
}
