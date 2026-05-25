// Branching on sign of an integer, covering an if / else-if / else
// chain that the structuring pass must recover.
public class IfElse {
    static String classify(int n) {
        if (n > 0) {
            return "pos";
        } else if (n < 0) {
            return "neg";
        } else {
            return "zero";
        }
    }

    public static void main(String[] args) {
        System.out.println(classify(3));
        System.out.println(classify(-2));
        System.out.println(classify(0));
    }
}
