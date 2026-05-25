public class StringOps {
    public static String repeat(String s, int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < n; i++) sb.append(s);
        return sb.toString();
    }

    public static int countChar(String s, char c) {
        int count = 0;
        for (int i = 0; i < s.length(); i++)
            if (s.charAt(i) == c) count++;
        return count;
    }

    public static void main(String[] args) {
        System.out.println(repeat("ab", 3));
        System.out.println(countChar("banana", 'a'));
        System.out.println("hello".toUpperCase());
        System.out.println(String.valueOf(42));
    }
}
