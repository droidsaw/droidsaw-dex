# R8 rules for the DCE-unreachable-branch fixture. Keep `compute` so
# R8's value-tracker can see FLAG → init() → false and prove the if
# branch unreachable, then strip it.
-allowaccessmodification
-keep public class ConstantCond {
    public static int compute(int);
}

# Preserve source-file + line-number tables so R8's mapping.txt
# carries inline attribution (line-range remapping is the oracle for
# `r8_inversion::recognise_method_inlined`).
-keepattributes SourceFile,LineNumberTable
