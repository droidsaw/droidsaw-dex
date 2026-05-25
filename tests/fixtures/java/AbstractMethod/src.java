// Abstract base class with a concrete subclass overriding the
// abstract method. Tests decompilation of abstract method
// declarations (body-less methods in the class_def).
abstract class Animal {
    private final String name;

    Animal(String name) {
        this.name = name;
    }

    String tag() {
        return name + ":" + sound();
    }

    abstract String sound();
}

class Dog extends Animal {
    Dog(String name) {
        super(name);
    }

    @Override
    String sound() {
        return "woof";
    }
}

public class AbstractMethod {
    public static void main(String[] args) {
        Animal a = new Dog("rex");
        System.out.println(a.tag());
    }
}
