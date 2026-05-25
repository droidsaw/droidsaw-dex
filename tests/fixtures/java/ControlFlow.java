public class ControlFlow {
    public static String sign(int n) {
        if (n > 0) return "pos";
        else if (n < 0) return "neg";
        else return "zero";
    }

    public static int sumTo(int n) {
        int s = 0;
        for (int i = 1; i <= n; i++) s += i;
        return s;
    }

    public static int collatz(int n) {
        int steps = 0;
        while (n != 1) {
            if (n % 2 == 0) n /= 2;
            else n = 3 * n + 1;
            steps++;
        }
        return steps;
    }

    public static String dayName(int d) {
        switch (d) {
            case 1: return "Mon";
            case 2: return "Tue";
            case 3: return "Wed";
            default: return "other";
        }
    }

    public static void main(String[] args) {
        System.out.println(sign(5));
        System.out.println(sign(-3));
        System.out.println(sign(0));
        System.out.println(sumTo(10));
        System.out.println(collatz(27));
        System.out.println(dayName(2));
        System.out.println(dayName(9));
    }
}
