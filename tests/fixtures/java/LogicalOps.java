// Short-circuit logical operators (&&, ||) — compile to branches in DEX.
public class LogicalOps {

    static boolean inRange(int x, int lo, int hi) {
        return x >= lo && x <= hi;
    }

    // Short-circuit with side effects
    static boolean bothPositive(int a, int b) {
        return a > 0 && b > 0;
    }

    static boolean eitherZero(int a, int b) {
        return a == 0 || b == 0;
    }

    public static void main(String[] args) {
        System.out.println(inRange(5, 1, 10));       // true
        System.out.println(inRange(0, 1, 10));       // false
        System.out.println(bothPositive(1, 2));       // true
        System.out.println(bothPositive(-1, 2));      // false
        System.out.println(eitherZero(0, 5));         // true
        System.out.println(eitherZero(3, 4));         // false
    }
}
