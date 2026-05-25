// Covers multi-catch: `catch (A | B | C e)` — one handler, multiple
// exception types, single binding. In Dalvik this compiles to multiple
// exception_handler entries pointing at the same handler_off; decompile
// must recognise the pattern and restore the `|`-joined type list
// (rather than emitting one catch clause per type or cloning the body).
public class MultiCatch {
    static String handle(int kind) {
        try {
            if (kind == 1) throw new IllegalStateException("ise");
            if (kind == 2) throw new NumberFormatException("nfe");
            if (kind == 3) throw new UnsupportedOperationException("uoe");
            return "none";
        } catch (IllegalStateException | NumberFormatException | UnsupportedOperationException e) {
            return e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(handle(0));
        System.out.println(handle(1));
        System.out.println(handle(2));
        System.out.println(handle(3));
    }
}
