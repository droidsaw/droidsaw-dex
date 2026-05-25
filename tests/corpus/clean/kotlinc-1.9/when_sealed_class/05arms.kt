// Sealed-class `when` over a sealed root with subclass subtypes — 5 arms.
// Above the 3-arm metadata threshold; recognizer fires.
// Lowering: linear chain of `aload + instanceof <Sub> + ifeq <next>` arms;
// `kotlin.NoWhenBranchMatchedException` exhaustiveness throw at fall-through.
// finding #2 in the kotlinc signature audit.

sealed class Shape {
    class Circle(val r: Int) : Shape()
    class Square(val s: Int) : Shape()
    class Triangle(val b: Int, val h: Int) : Shape()
    class Pentagon(val side: Int) : Shape()
    class Hexagon(val side: Int) : Shape()
}

fun area(sh: Shape): Int = when (sh) {
    is Shape.Circle -> sh.r * sh.r * 3
    is Shape.Square -> sh.s * sh.s
    is Shape.Triangle -> sh.b * sh.h / 2
    is Shape.Pentagon -> sh.side * sh.side * 2
    is Shape.Hexagon -> sh.side * sh.side * 3
}
