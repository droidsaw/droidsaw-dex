// adapted from R8 test src/test/examples/classmerging/ArrayTypeCollisionTest.java (Apache-2)
// Exercises method-overload resolution on covariant array parameter types.
// `class B extends A` makes `B[]` a subtype of `A[]`, so the overload
// `method(A[])` is potentially ambiguous with `method(B[])`. javac picks
// the most-specific signature at each call site; d8's smali round-trip
// must preserve the param-type so the decompiled call resolves the same.
public class ArrayOverload {
    public static void main(String[] args) {
        method(new A[] {});
        method(new B[] {});
    }

    private static void method(A[] obj) {
        System.out.println("In method(A[]), length: " + obj.length);
    }

    private static void method(B[] obj) {
        System.out.println("In method(B[]), length: " + obj.length);
    }

    public static class A {}
    public static class B extends A {}
}
