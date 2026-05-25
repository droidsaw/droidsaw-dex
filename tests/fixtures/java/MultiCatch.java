// Multi-catch and exception chaining.
public class MultiCatch {

    static String tryParse(String s) {
        try {
            int n = Integer.parseInt(s);
            return "int:" + n;
        } catch (NumberFormatException e) {
            return "bad:" + s;
        }
    }

    static int safeDivide(int a, int b) {
        try {
            return a / b;
        } catch (ArithmeticException e) {
            return -1;
        }
    }

    static String multiCatch(Object o) {
        try {
            String s = (String) o;
            return s.substring(0, 3);
        } catch (ClassCastException | StringIndexOutOfBoundsException e) {
            return "err";
        }
    }

    public static void main(String[] args) {
        System.out.println(tryParse("42"));       // int:42
        System.out.println(tryParse("abc"));      // bad:abc
        System.out.println(safeDivide(10, 3));    // 3
        System.out.println(safeDivide(10, 0));    // -1
        System.out.println(multiCatch("hello"));  // hel
        System.out.println(multiCatch(123));       // err
        System.out.println(multiCatch("ab"));      // err
    }
}
