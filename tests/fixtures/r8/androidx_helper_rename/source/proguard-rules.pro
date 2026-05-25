# R8 keep rules for the AndroidX-style library-helper rename fixture.
#
# Only the Main entry point is kept; the `androidx.testlib.*` helper
# class is intentionally NOT preserved with a wildcard keep, so R8
# fully minifies it. The result is a renamed `androidx.testlib.X`
# class with short-name methods (`a`, `b`, `c`, ...) that satisfy
# the BlockOutline recogniser's I4-I13 structural predicates by
# coincidence even though R8 has not outlined them.
#
# We intentionally do NOT add the standard `-keep class androidx.**
# { *; }` rule that real AndroidX consumers add via their AAR's
# bundled consumer-rules.pro. This fixture is demonstrating the
# specific case where an analyst would observe minified-not-kept
# library code in the wild and the recogniser would false-positive
# on it.

# Keep the entry point.
-keep class androidx.testlib.Main {
    public static void main(java.lang.String[]);
}

# Default R8 release behaviour: minify on, shrink on, optimize on.
# Outliner is enabled by default but will NOT fire here — the helper
# bodies are structurally distinct from each other so the outliner's
# >= 20 distinct callers per identical body predicate cannot match.

# Suppress notes for the bootstrap classes Kotlin's stdlib references
# but the fixture doesn't actually use.
-dontnote kotlin.**
-dontnote kotlinx.**

# Silence the "missing main class attribute" warning — d8/r8 emit it
# for our pre-packaged jar but the fixture is a library shape.
-dontwarn java.lang.invoke.StringConcatFactory
