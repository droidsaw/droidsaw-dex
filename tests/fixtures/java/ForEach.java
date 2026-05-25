// Enhanced for-each loops over arrays and collections.
public class ForEach {

    // Sum via for-each (int array)
    static int sum(int[] a) {
        int s = 0;
        for (int x : a) s += x;
        return s;
    }

    // Find max via for-each (int array)
    static int max(int[] a) {
        int m = a[0];
        for (int x : a) {
            if (x > m) m = x;
        }
        return m;
    }

    // Count occurrences of target via for-each
    static int count(int[] a, int target) {
        int c = 0;
        for (int x : a) {
            if (x == target) c++;
        }
        return c;
    }

    // Concatenate string array with separator
    static String join(String[] parts, String sep) {
        String result = "";
        boolean first = true;
        for (String p : parts) {
            if (!first) result += sep;
            result += p;
            first = false;
        }
        return result;
    }

    public static void main(String[] args) {
        int[] nums = {3, 1, 4, 1, 5};
        System.out.println(sum(nums));                  // 14
        System.out.println(max(nums));                  // 5
        System.out.println(count(nums, 1));             // 2
        String[] words = {"hello", "world"};
        System.out.println(join(words, " "));           // hello world
    }
}
