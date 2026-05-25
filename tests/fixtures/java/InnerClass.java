// Static inner class — tests that the $ filter is handled correctly
// and the inner class can be decompiled alongside the outer.
public class InnerClass {

    static int transform(int x) {
        return x * 2 + 1;
    }

    static int apply(int[] arr) {
        int sum = 0;
        for (int i = 0; i < arr.length; i++) {
            sum += transform(arr[i]);
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(transform(5));                    // 11
        System.out.println(apply(new int[]{1, 2, 3}));       // 15
    }
}
