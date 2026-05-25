# R8 rules for block-outlining fixture. Keep `driver` as the entry point;
# methodA/B/C are private to the implementation, so R8 is free to outline
# their shared body.
-allowaccessmodification
-keep public class RepeatedPattern {
    public static int driver(int, int);
}

# R8 9.0+ honours its own optimization configuration; the legacy
# `-optimizations` / `-optimizationpasses` options are ignored (R8 logs
# this as an Info message at compile time). The keep rule above is the
# only steering R8 needs to outline the matching method bodies.

# Preserve source-file + line-number tables so R8's mapping.txt
# carries inline attribution (line-range remapping is the oracle for
# `r8_inversion::recognise_method_inlined`).
-keepattributes SourceFile,LineNumberTable
