# R8 keep rules for the D8 desugar fixture.
#
# The fixture is a stand-alone Kotlin program; we keep Main entry +
# the API-entry-point classes, then let R8 OPTIMIZE the bodies. The
# critical point: we do NOT prevent R8 from outlining — the
# positive-control assertion requires R8 to outline at least the
# OutlineLike helpers into a non-j$ synthetic. We also do not force
# the j$/* backports to be preserved with -keep; D8 emits them and
# R8 propagates them as ordinary input classes.

# Keep the entry point.
-keep class fox.droidsaw.r8fixture.d8desugar.Main {
    public static void main(java.lang.String[]);
}

# Keep the API entry points by name so R8 doesn't inline them into
# main() (which would defeat the outliner threshold for the
# OutlineLike group). Bodies are still optimised under
# `,allowoptimization`.
-keep,allowoptimization class fox.droidsaw.r8fixture.d8desugar.TimeOps {
    public static java.lang.String t*(int);
}
-keep,allowoptimization class fox.droidsaw.r8fixture.d8desugar.StreamOps {
    public static java.lang.String s*(int);
}
-keep,allowoptimization class fox.droidsaw.r8fixture.d8desugar.OptionalOps {
    public static java.lang.String o*(int);
}
-keep,allowoptimization class fox.droidsaw.r8fixture.d8desugar.OutlineLike {
    public static java.lang.String n*(int);
}

# Default R8 release behaviour: minify on, shrink on, optimize on,
# outliner at default threshold = 20.

# Suppress notes for the bootstrap classes Kotlin's stdlib references
# but the fixture doesn't actually use.
-dontnote kotlin.**
-dontnote kotlinx.**

# Desugared-library namespace must NOT be aggressively shrunk away by
# R8 — the backports are reachable via the desugared call sites D8
# rewrote. Leave default reachability tracking on; `-dontwarn` covers
# any reference R8 cannot resolve at link time.
-dontwarn j$.**
-dontwarn java.lang.invoke.StringConcatFactory
