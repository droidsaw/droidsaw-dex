// One `for` loop accumulating a sum, one `while` loop counting
// iterations down. Covers loop-header recovery and back-edge
// handling for both shapes.
public class LoopBasic {
    static int sumTo(int n) {
        int s = 0;
        for (int i = 1; i <= n; i++) {
            s += i;
        }
        return s;
    }

    static int countdown(int n) {
        int seen = 0;
        while (n > 0) {
            seen++;
            n--;
        }
        return seen;
    }

    public static void main(String[] args) {
        System.out.println(sumTo(5));
        System.out.println(countdown(4));
    }
}
