// try-with-resources over a custom AutoCloseable. javac expands this
// into a nested try/finally with a suppressed-exception helper, so
// the decompiler's structuring pass has to recover the sugar.
class Resource implements AutoCloseable {
    private final String tag;

    Resource(String tag) {
        this.tag = tag;
        System.out.println("open:" + tag);
    }

    String read() {
        return "payload:" + tag;
    }

    @Override
    public void close() {
        System.out.println("close:" + tag);
    }
}

public class TryResources {
    static String consume(String tag) {
        try (Resource r = new Resource(tag)) {
            return r.read();
        }
    }

    public static void main(String[] args) {
        System.out.println(consume("a"));
    }
}
