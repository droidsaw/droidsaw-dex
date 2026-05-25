# R8 keep rules + minification knobs for the LIBRARY-HELPER-RENAME
# fixture. The shape mirrors Flutter's keep configuration
# (packages/flutter_tools/gradle/flutter_proguard_rules.pro), which is:
#
#   * -dontwarn on the io.flutter.** namespace (suppress missing-class
#     notes from optional plugin surfaces).
#   * -keep,allowshrinking,allowobfuscation on io.flutter.** (lets R8
#     RENAME the classes to short names; does NOT keep the original
#     names; does NOT keep methods structurally).
#
# Net effect: flutter_embedding's helper classes survive in the DEX
# (allowshrinking allows them to be DCE-ed if unused, but reachable
# ones are kept) but with SHORT MINIFIED NAMES. Nothing forces R8 to
# preserve the original class names, so io.flutter.plugin.platform.
# PlatformPlugin becomes io.flutter.a, io.flutter.b, etc.
#
# Apply the same shape here to MyLibraryHelpers so the fixture
# reproduces the empirical case: program input that R8 treats as
# library code, allowed to be renamed, NOT outlined.

# Keep the entry point. Main has to survive verbatim or the program
# is unreachable.
-keep class fox.droidsaw.r8fixture.libstub.Main {
    public static void main(java.lang.String[]);
}

# Flutter-style keep for the "library" helper. allowshrinking lets
# R8 drop unused members (none are unused here, but the flag matches
# Flutter's setup). allowobfuscation lets R8 rename the class and
# methods to short names. NOTE: no method-level signature pattern —
# R8 is free to rename every helper method's name + descriptor.
-keep,allowshrinking,allowobfuscation class fox.droidsaw.r8fixture.libstub.MyLibraryHelpers { *; }

# Suppress notes for Kotlin stdlib surfaces the fixture references
# transitively but doesn't otherwise touch.
-dontnote kotlin.**
-dontnote kotlinx.**

# Flutter's keep file uses -dontwarn liberally on io.flutter.**. The
# fixture analogue: silence warnings for the helper namespace + the
# StringConcatFactory bootstrap helper that Kotlin's compiler emits
# references to in some Java targets.
-dontwarn fox.droidsaw.r8fixture.libstub.**
-dontwarn java.lang.invoke.StringConcatFactory

# Default R8 release behaviour: minify on, shrink on, optimize on.
# The outliner is enabled by default in release mode, but it will
# NOT fire on MyLibraryHelpers because the helper bodies fail R8's
# identical-instruction-sequence gate (each helper does subtly
# different work — Integer.toHexString vs Integer.toBinaryString vs
# String.uppercase vs String.reversed vs raw append). That's the
# whole point: minification renames, outliner does not extract.
