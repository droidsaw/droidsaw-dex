// Aspirational Java that the Wave 2A `dex-r8-signatures` recogniser
// emits when decompiling the R8-inlined output of `SingleCall.java`.
// The original `clamp` is GONE from the DEX. The recogniser detects
// the inlined-clamp shape and annotates the `entry` body; it does NOT
// re-extract the helper (that requires a mapping file).
public class SingleCall {
    public static int entry(int x) {
        /* @droidsaw R8Origin(MethodInlined, source=$inlined) */
        int v = x * 2;
        int lo = 0;
        int hi = 100;
        if (v < lo) return lo;
        if (v > hi) return hi;
        return v;
    }
}
