public class EncodedValueInt {
    public static final int ZERO = 0;
    public static final int ONE_BYTE = 127;
    public static final int TWO_BYTE = 0x1234;
    public static final int THREE_BYTE = 0x123456;
    public static final int FOUR_BYTE = 0x12345678;
    public static final int POS_MAX = Integer.MAX_VALUE;
    public static final int NEG_ONE = -1;
    public static final int NEG_MAX = Integer.MIN_VALUE;

    public static void main(String[] args) {
        System.out.printf("zero=%d%n", ZERO);
        System.out.printf("one_byte=%d%n", ONE_BYTE);
        System.out.printf("two_byte=%d%n", TWO_BYTE);
        System.out.printf("three_byte=%d%n", THREE_BYTE);
        System.out.printf("four_byte=%d%n", FOUR_BYTE);
        System.out.printf("pos_max=%d%n", POS_MAX);
        System.out.printf("neg_one=%d%n", NEG_ONE);
        System.out.printf("neg_max=%d%n", NEG_MAX);
    }
}
