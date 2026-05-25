# EmptyClass

**Covers.** A class (`Holder`) with no explicit fields or methods — only the implicit default constructor appears in the DEX class_def. Exercises `decompile_class` on a minimal class_def and confirms an empty class recompiles cleanly.

**Status.** `compile_pass`. Full pipeline (javac → d8 → decompile → javac → java) is clean; recompiled program stdout matches `expected.txt` ("ok").
