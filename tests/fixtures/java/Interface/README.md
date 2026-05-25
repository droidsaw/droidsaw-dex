# Interface

**Covers.** An interface type with two implementations and an aggregator calling through the interface. Exercises `invoke-interface` dispatch plus decompilation of the interface's own class_def.

**Status.** `compile_pass` (graduated by the `dex-decompile-interface-covariance` stream).

**Fix.** The earlier `compile_fail` was misdiagnosed as an array-covariance bug. The real defect was that `src/classes.rs::decompile_class` hard-coded the type keyword as `class` and ignored `ACC_INTERFACE` (`0x0200`) on the class_def, so `interface Shape` was rendered as `abstract class Shape`. That broke the implementing classes: `Square implements Shape` / `Rectangle implements Shape` is illegal when `Shape` is a class (Java requires `extends` for class-to-class, `implements` for class-to-interface, `extends` for interface-to-interface). The "incompatible types: Square cannot be converted to Shape" error was the cascading javac rejection, not a widening failure — Java accepts implicit subtype-to-supertype widening on array-element assignment when the target type is correctly resolved as a supertype (which `interface Shape` is and `abstract class Shape` is not when the relationship is `implements`).

The fix detects `ACC_INTERFACE` and emits the appropriate type keyword + parent-clause spelling:

- `interface Foo` (not `abstract class Foo` — the `abstract` modifier is implicit on interfaces, and Java rejects it as redundant alongside `interface`)
- `extends Bar, Baz` for parent interfaces (an interface `extends` other interfaces; it does not `implements` them)
- `extends` suppression for the implicit `java.lang.Object` supertype (interfaces' Dalvik `superclass_idx` is always Object; classes already skip it; interfaces follow the same rule)

Orthogonal decompile tasks still out of scope:
- Annotation types (`ACC_ANNOTATION` → `@interface`) and enums (`ACC_ENUM` → `enum`) follow the same access-flag-driven pattern; left as separate streams (see `dex-decompile-nested-classes` for enum-with-methods, which is an umbrella).
- Genuine array-covariance narrowing cases (if a future fixture surfaces one that isn't explained by the type-keyword gap) would need their own stream.
