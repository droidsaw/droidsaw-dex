// adapted from R8 test src/test/examples/classmerging/ClassWithNativeMethodTest.java (Apache-2)
// Exercises the `native` modifier on a method declaration. javac emits
// ACC_NATIVE (0x0100) and zero bytecode; d8 must preserve the access
// flag and the empty body. `args.length == 42` is a guaranteed-false
// guard so the program never tries to load a JNI library at runtime —
// the corner being tested is the IR/emit shape of a native declaration,
// not its dynamic behavior. Expected stdout is empty.
public class NativeMethod {
    public static void main(String[] args) {
        B obj = new B();
        if (args.length == 42) {
            obj.method();
        }
    }

    public static class A {
        public native void method();
    }

    public static class B extends A {}
}
