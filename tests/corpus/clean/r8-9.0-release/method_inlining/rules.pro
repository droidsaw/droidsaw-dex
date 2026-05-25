# R8 rules for method-inlining fixture. Keep only `entry`; `clamp` is
# left unconstrained so R8 is free to inline it.
-allowaccessmodification
-keep public class SingleCall {
    public static int entry(int);
}

# Preserve source-file + line-number tables so R8's mapping.txt
# carries inline attribution (line-range remapping is the oracle for
# `r8_inversion::recognise_method_inlined`).
-keepattributes SourceFile,LineNumberTable
