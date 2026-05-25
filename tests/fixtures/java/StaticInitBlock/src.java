// Covers a static initializer block. Dalvik emits <clinit> as a
// separate method alongside <init>; decompile must recognise it as
// `static { ... }` syntax rather than inline its body into the default
// constructor or emit it as an ordinary static method.
public class StaticInitBlock {
    static final int X;
    static final int Y;

    static {
        X = 10;
        Y = X * 2;
    }

    public static void main(String[] args) {
        System.out.println(X);
        System.out.println(Y);
    }
}
