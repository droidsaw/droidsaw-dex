// Instanceof checks and type casting.
public class InstanceOf {

    static String describe(Object o) {
        if (o instanceof String) {
            String s = (String) o;
            return "str:" + s.length();
        } else if (o instanceof Integer) {
            int n = (Integer) o;
            return "int:" + n;
        } else if (o == null) {
            return "null";
        }
        return "other";
    }

    static int sumIntegers(Object[] arr) {
        int sum = 0;
        for (Object o : arr) {
            if (o instanceof Integer) {
                sum += (Integer) o;
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println(describe("hello"));     // str:5
        System.out.println(describe(42));           // int:42
        System.out.println(describe(null));         // null
        System.out.println(describe(3.14));         // other
        Object[] mix = {"a", 1, "b", 2, 3};
        System.out.println(sumIntegers(mix));       // 6
    }
}
