// Static fields, class init, read/write static state.
public class StaticState {

    static int counter = 0;
    static final int MULTIPLIER = 7;
    static final String PREFIX = "val:";

    static void increment() {
        counter++;
    }

    static int getScaled() {
        return counter * MULTIPLIER;
    }

    static String format(int x) {
        return PREFIX + x;
    }

    public static void main(String[] args) {
        System.out.println(counter);       // 0
        increment();
        increment();
        increment();
        System.out.println(counter);       // 3
        System.out.println(getScaled());   // 21
        System.out.println(format(42));    // val:42
    }
}
