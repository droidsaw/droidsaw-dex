// Standard try/catch with a finally clause. Covers exception-region
// recovery and the structuring pass's conversion of Dalvik catch
// tables back to Java try blocks.
public class TryCatch {
    static int parseOr(String s, int fallback) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    static int divFinally(int a, int b) {
        try {
            return a / b;
        } catch (ArithmeticException e) {
            return -1;
        } finally {
            System.out.println("done");
        }
    }

    public static void main(String[] args) {
        System.out.println(parseOr("42", 0));
        System.out.println(parseOr("xy", -7));
        System.out.println(divFinally(10, 2));
        System.out.println(divFinally(1, 0));
    }
}
