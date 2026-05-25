// Multi-dimensional arrays: creation, nested access, iteration.
public class MultiArray {

    // Create and fill a 2D identity matrix
    static int[][] identity(int n) {
        int[][] m = new int[n][n];
        for (int i = 0; i < n; i++) {
            m[i][i] = 1;
        }
        return m;
    }

    // Matrix-vector multiply: m[r][c] * v[c]
    static int[] matvec(int[][] m, int[] v) {
        int[] result = new int[m.length];
        for (int i = 0; i < m.length; i++) {
            int sum = 0;
            for (int j = 0; j < v.length; j++) {
                sum += m[i][j] * v[j];
            }
            result[i] = sum;
        }
        return result;
    }

    // Sum all elements of a 2D array
    static int sum2d(int[][] a) {
        int s = 0;
        for (int[] row : a) {
            for (int x : row) {
                s += x;
            }
        }
        return s;
    }

    public static void main(String[] args) {
        int[][] id = identity(3);
        System.out.println(id[0][0] + " " + id[0][1]);      // 1 0
        System.out.println(id[1][1]);                         // 1

        int[] v = {2, 3, 4};
        int[] r = matvec(id, v);
        System.out.println(r[0] + " " + r[1] + " " + r[2]); // 2 3 4

        int[][] grid = {{1, 2}, {3, 4}, {5, 6}};
        System.out.println(sum2d(grid));                      // 21
    }
}
