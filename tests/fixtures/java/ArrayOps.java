public class ArrayOps {
    public static int sum(int[] a) {
        int s = 0;
        for (int x : a) s += x;
        return s;
    }

    public static void reverse(int[] a) {
        for (int i = 0, j = a.length - 1; i < j; i++, j--) {
            int t = a[i]; a[i] = a[j]; a[j] = t;
        }
    }

    public static void main(String[] args) {
        int[] a = {1, 2, 3, 4, 5};
        System.out.println(sum(a));
        reverse(a);
        for (int x : a) System.out.print(x + " ");
        System.out.println();
        int[] b = new int[3];
        b[0] = 10; b[1] = 20; b[2] = 30;
        System.out.println(sum(b));
    }
}
