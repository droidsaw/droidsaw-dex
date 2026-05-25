// Primitive casts and type conversions — exercises DEX int-to-*, *-to-int,
// wide conversions, and narrowing casts that javac emits explicitly.
public class Casts {

    // Widening: int → long → double
    static double widen(int x) {
        long l = x;
        double d = l;
        return d + 0.5;
    }

    // Narrowing: long → int, double → int (truncation)
    static int narrow(long l, double d) {
        int a = (int) l;
        int b = (int) d;
        return a + b;
    }

    // Sub-int narrowing: int → byte, int → short, int → char
    static String subInt(int x) {
        byte b = (byte) x;
        short s = (short) x;
        char c = (char) x;
        return b + " " + s + " " + c;
    }

    // Float ↔ double
    static float floatRound(double d) {
        float f = (float) d;
        return f;
    }

    // int → float (lossy for large values, but semantically correct)
    static float intToFloat(int x) {
        return (float) x;
    }

    // Chained cast: int → long → float → int
    static int chainCast(int x) {
        long l = x;
        float f = (float) l;
        return (int) f;
    }

    public static void main(String[] args) {
        System.out.println(widen(10));                  // 10.5
        System.out.println(narrow(100000L, 3.99));      // 100003
        System.out.println(subInt(65));                 // 65 65 A
        System.out.println(floatRound(2.5));            // 2.5
        System.out.println(intToFloat(42));             // 42.0
        System.out.println(chainCast(12345));           // 12345
    }
}
