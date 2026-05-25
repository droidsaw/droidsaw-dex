public class Arithmetic {
    public static int add(int a, int b) { return a + b; }
    public static int mul(int a, int b) { return a * b; }
    public static int mod(int a, int b) { return a % b; }
    public static long addLong(long a, long b) { return a + b; }

    public static void main(String[] args) {
        System.out.println(add(3, 4));
        System.out.println(mul(6, 7));
        System.out.println(mod(17, 5));
        System.out.println(addLong(1000000000L, 2000000000L));
    }
}
