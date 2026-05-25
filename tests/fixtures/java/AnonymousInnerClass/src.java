// Covers an anonymous inner class (`new Runnable() { ... }`). javac
// compiles this as a named synthetic class `AnonymousInnerClass$1` with
// a synthetic constructor that captures any referenced effectively-final
// local as a synthetic constructor argument + stored field. Decompile
// must recognise the pattern and restore `new Runnable() { ... }` rather
// than emitting the synthetic class as a standalone top-level form.
public class AnonymousInnerClass {
    public static void main(String[] args) {
        final String msg = "hello";
        Runnable r = new Runnable() {
            @Override
            public void run() {
                System.out.println(msg);
            }
        };
        r.run();
    }
}
