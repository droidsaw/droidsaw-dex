// Aspirational Java that the Wave 2A `dex-r8-signatures` recogniser
// emits when decompiling the R8-DCE'd output. The dead branch is
// genuinely GONE from the bytecode; the recogniser annotates the
// surviving compute body to flag that DCE happened (heuristic, no
// way to recover the stripped text).
public class ConstantCond {
    /* @droidsaw R8Origin(DeadBranchStripped, helper=$dce-cue) */
    public static int compute(int x) {
        return x * 2;
    }
}
