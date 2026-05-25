// A small leaf helper method called from a single call site. R8's
// method-inlining heuristic inlines small leaf methods at every call
// site; with only one call site, the helper is removed entirely after
// inlining.
//
// Recogniser intent (Wave 2A `dex-r8-signatures`): the post-R8 DEX has
// NO trace of `clamp`; only `entry`'s body, which now contains the
// inlined arithmetic. Without an R8 `--mapping-file` there is no way
// to recover that `clamp` ever existed — this transformation is
// "lossy by construction". The recogniser is annotation-only:
// `R8Origin { variant: R8Transform::MethodInlined, source_pc, ... }`
// flags the suspected inline site without renaming anything.
public class SingleCall {
    private static int clamp(int v, int lo, int hi) {
        if (v < lo) return lo;
        if (v > hi) return hi;
        return v;
    }

    public static int entry(int x) {
        return clamp(x * 2, 0, 100);
    }
}
