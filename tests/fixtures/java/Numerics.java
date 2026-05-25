// Basic numeric algorithms — builds up incrementally as decompiler bugs are fixed.
public class Numerics {

    // Integer square root via Newton's method (exercises: phi-from-param init,
    // expression-inlining precedence, loops where both variables update each iter).
    static int isqrt(int n) {
        int x = n;
        int y = (x + 1) / 2;
        while (y < x) {
            x = y;
            y = (x + n / x) / 2;
        }
        return x;
    }

    // Power of 2 (simple loop, const-init phis)
    static int pow2(int n) {
        int r = 1;
        for (int i = 0; i < n; i++) r *= 2;
        return r;
    }

    // Dot product of two scalars
    static int dot2(int a, int b, int c, int d) {
        return a * b + c * d;
    }

    // Sum via explicit index loop (exercises a.length-bound loop with aget in body)
    static int isum(int[] a) {
        int s = 0;
        for (int i = 0; i < a.length; i++) {
            s += a[i];
        }
        return s;
    }

    // long const-init phi, long add, long return (wide-type register pair)
    static long sumLong(int n) {
        long s = 0L;
        for (int i = 1; i <= n; i++) s += i;
        return s;
    }

    // int-to-double cast, double accumulate, double return
    static double mean(int[] a) {
        double s = 0.0;
        for (int x : a) s += x;
        return s / a.length;
    }

    public static void main(String[] args) {
        System.out.println(isqrt(2));               // 1
        System.out.println(isqrt(9));               // 3
        System.out.println(pow2(5));                // 32
        System.out.println(dot2(1, 1, 1, 2));      // 3
        System.out.println(isum(new int[]{1,2,3,4,5})); // 15
        System.out.println(sumLong(100));           // 5050
        System.out.println(mean(new int[]{1,2,3,4,5})); // 3.0
    }
}
