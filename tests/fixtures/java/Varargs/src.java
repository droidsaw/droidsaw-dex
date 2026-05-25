// Covers varargs — `void f(int... xs)`. Dalvik lowers the call site to
// array-new + fill-array-data + invoke, and the method itself gets an
// `ACC_VARARGS` access flag plus a last parameter of array type.
// Decompile must restore the `...` in the signature from the access
// flag (not emit the method as taking an explicit array) and
// reconstruct the `f(1,2,3)` call site from the array-fill sequence.
public class Varargs {
    static void show(String label, int... nums) {
        int sum = 0;
        for (int n : nums) sum += n;
        System.out.println(label + ":" + sum);
    }

    public static void main(String[] args) {
        show("a", 1);
        show("b", 1, 2, 3);
        show("c");
    }
}
