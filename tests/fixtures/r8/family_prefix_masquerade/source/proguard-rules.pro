# Adversarial PoC keep rules: preserve the masquerade surface.
#
# The fixture must end with a class whose FQCN starts with
# `androidx.adversarial.poc.*` in the final DEX — that's the
# masquerade premise. R8's default behaviour with
# `allowobfuscation` is to move minified classes to the root
# package (e.g. Backdoor -> a, losing the androidx.* prefix
# entirely). To preserve the namespace:
#
#   * Plain `-keep` on Backdoor — no `allowobfuscation`, no
#     `allowshrinking`. Class name AND package stay verbatim, and
#     the static helper method survives R8's tree-shake.
#   * `-keeppackagenames` forbids repackaging of every class under
#     the masquerade prefix. Defence in depth against R8 inferring
#     `-flattenpackagehierarchy`.
#   * Plain `-keep` on the 25 caller methods so they survive as
#     distinct caller methods (satisfies the recogniser's
#     distinct_caller_count gate).
#   * `-dontoptimize` blocks single-call inlining; without it R8
#     would inline Backdoor.execute into all 25 callers,
#     dissolving the helper.

# The masquerade premise is "class FQCN starts with androidx.* in the
# final DEX." R8 9.x has been observed to rename classes despite
# `-keep`, `-keepnames`, and `-keeppackagenames` on this fixture
# (likely because Kotlin object members emit dual signatures and the
# kept rule matches the wrong one, freeing R8 to rename the class).
#
# The reliable hammer is `-dontobfuscate`: disables R8's obfuscator
# entirely. The class names are then preserved verbatim, which is
# actually MORE faithful to a realistic adversarial scenario — an
# attacker who has chosen masquerade names wants them readable so
# casual FQCN-skimming analysts don't notice the squat.
-dontobfuscate
-dontoptimize
-dontwarn **

-keep public class androidx.adversarial.poc.Main {
    public static void main(java.lang.String[]);
}

-keep class androidx.adversarial.poc.Backdoor {
    public static java.lang.String execute(int, java.lang.String);
}

-keep class androidx.adversarial.poc.Main {
    public void call**();
}
