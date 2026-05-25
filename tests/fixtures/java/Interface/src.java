// An interface with two implementations plus a caller dispatching
// via the interface type. Covers invoke-interface and abstract
// method recovery for the interface class_def.
interface Shape {
    int area();
}

class Square implements Shape {
    private final int side;

    Square(int side) {
        this.side = side;
    }

    @Override
    public int area() {
        return side * side;
    }
}

class Rectangle implements Shape {
    private final int w;
    private final int h;

    Rectangle(int w, int h) {
        this.w = w;
        this.h = h;
    }

    @Override
    public int area() {
        return w * h;
    }
}

public class Interface {
    static int totalArea(Shape[] shapes) {
        int sum = 0;
        for (Shape s : shapes) {
            sum += s.area();
        }
        return sum;
    }

    public static void main(String[] args) {
        Shape[] shapes = new Shape[] {
            new Square(3),
            new Rectangle(2, 5),
        };
        System.out.println(totalArea(shapes));
    }
}
