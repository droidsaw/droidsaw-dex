# EnumWithMethods

**Covers.** A Java enum with per-constant method bodies (`ADD { int apply(...) {...} }`, `SUB { int apply(...) {...} }`). javac compiles each constant's body as an anonymous subclass of the enum type, so the dex corpus contains four class_defs: `EnumWithMethods`, `EnumWithMethods$Op`, `EnumWithMethods$Op$1` (ADD's body), `EnumWithMethods$Op$2` (SUB's body).

**Status.** `compile_fail` at `Recompile` stage.

**Decompiler gap.** Same nested-class-concat gap as StaticNestedClass / AnonymousInnerClass / LocalInnerClass: the enum type and its per-constant subclasses decompile in isolation but are not included in the outer fixture's recompile set:

```
EnumWithMethods.java:9: error: cannot find symbol
    EnumWithMethods$Op v0 = EnumWithMethods$Op.ADD;
    ^
  symbol:   class EnumWithMethods$Op
  location: class EnumWithMethods
```

Additional enum-specific restoration (masked by the nested-emission error): the per-constant subclasses need to be restored to `ADD { ... }, SUB { ... }` syntax inside the enum body, not left as separately-emitted anonymous subclasses. This second-order step becomes visible only after the nested-emission fix lands and the fixture progresses to a different failure mode.

**Candidate follow-up streams.** Primary: `dex-decompile-nested-classes` (same umbrella as the 3 sibling nested-class fixtures). Secondary (downstream): `dex-decompile-enum-per-constant-body` — pattern-match the enum-constants-as-anonymous-subclasses shape and restore the `CONSTANT { body }` syntax.
