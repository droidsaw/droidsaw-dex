// `if (FLAG) { … }` where FLAG is a non-constant static (defeats javac
// constant-folding) but is provably false at R8's analysis horizon.
// R8 traces the field to its writer in `<clinit>` and proves the branch
// is unreachable, then strips the dead arm entirely.
//
// Recogniser intent (Wave 2A `dex-r8-signatures`): detect orphan SSA
// defs / unreachable-block remnants that survive R8's stripping.
// Annotation-only — there's no way to recover the original branch text
// from the post-DCE bytecode.
public class ConstantCond {
    // Not a compile-time constant — `init()` defeats javac folding.
    private static final boolean FLAG = init();

    private static boolean init() {
        // Returns false unconditionally; R8 traces this and folds FLAG.
        return false;
    }

    public static int compute(int x) {
        if (FLAG) {
            // R8 strips this branch.
            return -1;
        }
        return x * 2;
    }
}
