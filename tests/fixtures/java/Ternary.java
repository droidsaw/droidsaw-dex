// Ternary / conditional expressions — diamond CFG pattern.
public class Ternary {

    static int abs(int x) {
        return x >= 0 ? x : -x;
    }

    static int clamp(int x, int lo, int hi) {
        return x < lo ? lo : (x > hi ? hi : x);
    }

    static int min(int a, int b) {
        return a < b ? a : b;
    }

    static int max3(int a, int b, int c) {
        int m = a > b ? a : b;
        return m > c ? m : c;
    }

    public static void main(String[] args) {
        System.out.println(abs(-7));           // 7
        System.out.println(abs(3));            // 3
        System.out.println(clamp(5, 0, 10));   // 5
        System.out.println(clamp(-1, 0, 10));  // 0
        System.out.println(clamp(99, 0, 10));  // 10
        System.out.println(min(3, 7));         // 3
        System.out.println(max3(1, 5, 3));     // 5
    }
}
