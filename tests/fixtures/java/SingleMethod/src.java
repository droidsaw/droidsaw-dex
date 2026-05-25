// A minimal class exposing one static helper plus main. Covers
// single-method decompile output and invoke-static dispatch.
public class SingleMethod {
    static int square(int n) {
        return n * n;
    }

    public static void main(String[] args) {
        System.out.println(square(7));
    }
}
