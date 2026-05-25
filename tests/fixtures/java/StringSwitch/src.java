// Covers Java 7+ `switch(String)`. javac lowers this to a two-level
// form: an outer `switch` on `s.hashCode()` whose cases `equals`-check
// the actual string and dispatch to a synthetic int tag; a second
// switch on the tag runs the user cases. Decompile must recognise the
// hash+equals+tag-switch pattern and restore the single `switch(s)`
// form over string case labels.
public class StringSwitch {
    static int numberOf(String s) {
        switch (s) {
            case "one":   return 1;
            case "two":   return 2;
            case "three": return 3;
            default:      return 0;
        }
    }

    public static void main(String[] args) {
        System.out.println(numberOf("one"));
        System.out.println(numberOf("two"));
        System.out.println(numberOf("three"));
        System.out.println(numberOf("four"));
    }
}
