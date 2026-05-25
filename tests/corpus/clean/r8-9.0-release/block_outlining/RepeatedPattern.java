// Three methods with structurally identical bodies. R8's block-outlining
// optimisation hoists the repeated bytecode sequence into a synthetic
// helper method and replaces the three originals with single-call
// trampolines into the helper.
//
// Recogniser intent (Wave 2A `dex-r8-signatures`): detect a method whose
// body is a single `invoke-static $synthetic-outlined-N` call, lift to
// `Stmt::OutlinedBlock { synthetic_target: MethodIdx, origin: R8Origin {
// variant: R8Transform::BlockOutlined, ... } }`. The synthetic helper's
// body reveals the original block shape.
public class RepeatedPattern {
    public static int methodA(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    public static int methodB(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    public static int methodC(int x, int y) {
        int r = x * 17;
        r = r + y * 23;
        r = r ^ 0x5A5A5A5A;
        r = r >>> 3;
        return r + 42;
    }

    public static int driver(int x, int y) {
        return methodA(x, y) + methodB(x, y) + methodC(x, y);
    }
}
