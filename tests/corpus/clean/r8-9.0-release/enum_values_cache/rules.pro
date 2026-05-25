# R8 rules for enum-values-cache fixture. Keep all three public entry
# points so R8 sees the repeated `values()` call sites and lifts them.
-allowaccessmodification
-keep public class Simple {
    public static int countRed();
    public static int countGreen();
    public static int countBlue();
}

# Allow R8 to optimise enum types — without this, R8 may treat enums
# as opt-out-of-optimisation by default.
-keepclassmembers,allowoptimization enum * {
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# Preserve source-file + line-number tables so R8's mapping.txt
# carries inline attribution (line-range remapping is the oracle for
# `r8_inversion::recognise_method_inlined`).
-keepattributes SourceFile,LineNumberTable
