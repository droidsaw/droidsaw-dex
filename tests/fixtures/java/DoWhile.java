// Nested loops and loop variants.
public class DoWhile {

    // Nested loop: multiplication table sum
    static int tableSum(int rows, int cols) {
        int sum = 0;
        for (int i = 1; i <= rows; i++) {
            for (int j = 1; j <= cols; j++) {
                sum += i * j;
            }
        }
        return sum;
    }

    // Triple-nested accumulation
    static int cube(int n) {
        int sum = 0;
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                for (int k = 0; k < n; k++) {
                    sum++;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(tableSum(3, 4));    // 60
        System.out.println(cube(3));           // 27
    }
}
