// Exercises return-position type inference for const-zero defs:
//   `const/4 0; return v` in a boolean-returning method must emit `return false;`
//   `const/4 0; return-object v` in a reference-returning method must emit `return null;`
// Without the guard the decompiler types the const def as `Int` (resolve_one_const had no
// arm for Return / ReturnObject), so emit_expr's null-fold and Boolean-fold
// never fired and rendered `return 0;` — invalid Java for both shapes.
public class BooleanAndNullReturns {
    static boolean alwaysFalse() {
        return false;
    }

    static boolean alwaysTrue() {
        return true;
    }

    static Object alwaysNull() {
        return null;
    }

    static String maybeNull(int x) {
        if (x == 0) {
            return null;
        }
        return "x";
    }

    public static void main(String[] args) {
        System.out.println(alwaysFalse());
        System.out.println(alwaysTrue());
        System.out.println(alwaysNull());
        System.out.println(maybeNull(0));
        System.out.println(maybeNull(1));
    }
}
