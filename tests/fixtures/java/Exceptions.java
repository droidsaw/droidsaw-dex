// Companion class — package-private so AppError lives in the same DEX
// without becoming an inner class (LExceptions$AppError).
class AppError extends RuntimeException {
    AppError(String msg) { super(msg); }
}

public class Exceptions {

    static int parse(String s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            throw new AppError("bad");
        }
    }

    // try / catch / finally
    static int safeParse(String s) {
        try {
            return parse(s);
        } catch (AppError e) {
            return -1;
        } finally {
            // exercises finally-block codegen; no visible side-effect
        }
    }

    // single catch, returns String
    static String divide(int a, int b) {
        try {
            return String.valueOf(a / b);
        } catch (ArithmeticException e) {
            return "divzero";
        }
    }

    public static void main(String[] args) {
        System.out.println(safeParse("42"));   // 42
        System.out.println(safeParse("??"));   // -1
        System.out.println(divide(10, 2));     // 5
        System.out.println(divide(7, 0));      // divzero
    }
}
