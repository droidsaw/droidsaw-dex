// javac-21 corpus seed: try-with-resources, single resource.
// javac lowers TWR to a hidden try/catch with primaryExc tracking +
// Throwable.addSuppressed on the close path. Reconstructed as
// Stmt::TryWithResources (Stream #4 introduces the variant).
import java.io.StringReader;
class T {
    static int f() throws Exception {
        try (StringReader r = new StringReader("hello")) {
            return r.read();
        }
    }
}
