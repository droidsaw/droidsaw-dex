# R8 rules for the internal-ratchet fixture. Keep the public driver
# entry points + count* methods so R8 sees the repeated mix* and
# Color.values() call patterns.
-allowaccessmodification

-keep public class InternalRatchet {
    public static int driverA(int, int);
    public static int driverB(int, int);
    public static int driverC(int, int);
    public static int controlSimple(int);
    public static int controlBranchy(int);
    public static int countRed();
    public static int countGreen();
    public static int countBlue();
    public static int countAlpha();
    public static int countWhite();
    public static int countBlack();
}

# Allow R8 to optimise enum types — without this, R8 may treat
# enums as opt-out-of-optimisation by default and skip the
# enum-unboxing synthetic helper that the ratchet exercises.
-keepclassmembers,allowoptimization enum * {
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# Preserve source-file + line-number tables so mapping.txt carries
# inline attribution. The ratchet keys off the
# `com.android.tools.r8.synthesized` annotation but the keep is
# uniform with the other fixtures.
-keepattributes SourceFile,LineNumberTable
