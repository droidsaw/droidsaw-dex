// Early return inside loops — post-loop code must not be lost.
public class EarlyReturn {

    static int firstOver(int[] arr, int threshold) {
        for (int i = 0; i < arr.length; i++) {
            if (arr[i] > threshold) return arr[i];
        }
        return -1;
    }

    static int indexOf(int[] arr, int target) {
        for (int i = 0; i < arr.length; i++) {
            if (arr[i] == target) return i;
        }
        return -1;
    }

    public static void main(String[] args) {
        int[] a = {1, 5, 3, 8, 2};
        System.out.println(firstOver(a, 4));    // 5
        System.out.println(firstOver(a, 10));   // -1
        System.out.println(indexOf(a, 3));      // 2
        System.out.println(indexOf(a, 9));      // -1
    }
}
