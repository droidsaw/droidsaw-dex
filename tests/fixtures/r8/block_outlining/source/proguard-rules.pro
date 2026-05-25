# R8 keep rules + outliner knobs for the BlockOutline fixture.
#
# The fixture is a stand-alone Kotlin program; we want R8 to keep the
# Main entry point + all FixtureCallers methods reachable, then
# OPTIMIZE the bodies — specifically letting the outliner fire on
# the repeated StringBuilder sequence shared by every caller.

# Keep the entry point.
-keep class fox.droidsaw.r8fixture.Main {
    public static void main(java.lang.String[]);
}

# Keep every caller method across the three groups by name +
# signature so R8 doesn't inline them into main() (which would
# defeat the outliner's >= 20 distinct callers gate). Keeping by
# name only — bodies are still optimised so the outliner can lift
# the shared StringBuilder body per group.
-keep,allowoptimization class fox.droidsaw.r8fixture.GroupAInt {
    public static java.lang.String a*(int);
}
-keep,allowoptimization class fox.droidsaw.r8fixture.GroupBLong {
    public static java.lang.String b*(long);
}
-keep,allowoptimization class fox.droidsaw.r8fixture.GroupCStr {
    public static java.lang.String c*(java.lang.String);
}

# Default R8 release behaviour: minify on, shrink on, optimize on.
# Outliner is enabled by default in release mode at OutlineOptions
# defaults (threshold = 20). No special flag needed.

# Suppress notes for the bootstrap classes Kotlin's stdlib references
# but the fixture doesn't actually use.
-dontnote kotlin.**
-dontnote kotlinx.**

# Silence the "missing main class attribute" warning — d8/r8 emit it
# for our pre-packaged jar but the fixture is a library shape.
-dontwarn java.lang.invoke.StringConcatFactory
