# LocalInnerClass

**Covers.** A method-local inner class: `class Adder { ... }` declared inside `main`. javac compiles this to `LocalInnerClass$1Adder` (the `$1` prefix indexes method-local classes per enclosing member) with synthetic fields holding captured effectively-final locals.

**Status.** `compile_pass` (graduated by the `dex-decompile-nested-classes` stream).

**Fix.** Same root cause + fix as StaticNestedClass / AnonymousInnerClass: `tests/fixture_ratchet.rs::decompile` was filtering `cls.descriptor.contains('$')`, skipping the synthetic `LocalInnerClass$1Adder` class. Removing the filter writes it to its own `<name>.java` file in the recompile src dir; javac accepts the `$1Adder` identifier and resolves references from the outer.

**Out of scope.** Source-faithful restoration would place the `class Adder { ... }` declaration INSIDE the `main(String[] args)` method body (matching the original Java source's scoping). Current output emits it as a sibling top-level `class LocalInnerClass$1Adder { ... }` file — compiles + runs correctly, matches expected stdout, but less source-faithful. Enclosing-method restoration would read the `EnclosingMethod` annotation attribute on the inner's class_def and emit the declaration inside that method's body. Future polish stream, not required for fixture graduation.
