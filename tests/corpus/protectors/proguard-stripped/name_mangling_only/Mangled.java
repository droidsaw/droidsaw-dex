// Synthetic ProGuard-stripped baseline — class names mangled to single
// letters, methods reduced to short opaque tokens, debug info absent.
// NO adversarial signatures present:
//   - No fragmented string literal concat (StringConcat with all-Literal
//     parts) — every literal is whole.
//   - No reflective binder-stub call — no Class.forName / Method.invoke.
//   - No flattened dispatcher / opaque predicate / encrypted decryptor.
//
// The protector recognizers (SignatureId 200, 201, 202, ...) MUST NOT
// fire on this shape. Production R8-processed APK UNRECOGNIZED_REGION ratchet
// (baseline=3) is the empirical gate; this fixture documents the
// negative-gate intent.
//
// Negative-gate intent: no positive matches on obfuscated-only names.
//
// Compile + d8 are NOT auto-driven by `tests/corpus_check.rs` for the
// protectors/ tree; this source is a documentation artifact paired with
// signature.toml. A future protectors corpus harness can add the
// auto-compile path if a positive gate becomes useful.

class M {
    int a;
    String b;

    M(int x, String y) {
        this.a = x;
        this.b = y;
    }

    int f(int n) {
        if (n <= 1) return 1;
        return n * f(n - 1);
    }

    String g(String s) {
        return s + "!";
    }

    public static void main(String[] args) {
        M m = new M(5, "hello");
        System.out.println(m.f(m.a) + " " + m.g(m.b));
    }
}
