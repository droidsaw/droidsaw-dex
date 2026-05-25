// Fixture exercising debug_info-retained LOCAL variable names. Compile
// with `javac -g -parameters` to keep LocalVariableTable populated.
// droidsaw-dex's name-propagation loop is expected to rename the SSA
// `vN_M` placeholders to `counter`, `total`, `builder` in the decompile
// output. Multi-use locals (reads inside a loop body + final return)
// resist the optimizer's single-use inlining pass so the names stay
// visible in the decompile output.
public class MinimalNamed {
    public static long accumulate(int input) {
        long counter = 0;
        long total = input;
        for (int i = 0; i < input; i++) {
            counter = counter + 1;
            total = total + counter;
        }
        return total + counter;
    }
}
