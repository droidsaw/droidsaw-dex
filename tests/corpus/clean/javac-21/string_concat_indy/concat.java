// javac-21 corpus seed: invokedynamic StringConcatFactory lowering.
// javac 9+ lowers `"a" + b + "c"` to a single
// `invokedynamic StringConcatFactory.makeConcatWithConstants` call site,
// not the pre-Java-9 StringBuilder.append() chain. The recipe string
// (first VALUE_STRING in the call_site's encoded_array) carries the
// literal segments interleaved with `` placeholders for runtime
// arguments.
class T {
    static String greet(String name, int n) {
        return "Hello, " + name + "! You have " + n + " messages.";
    }

    static String simple(String s) {
        // Single-argument form — recipe is "<>" with one placeholder.
        return "<" + s + ">";
    }
}
