public class EncodedValueLong {
    public static final long ZERO = 0L;
    public static final long ONE_BYTE = 127L;
    public static final long FOUR_BYTE = 0x12345678L;
    public static final long FIVE_BYTE = 0x123456789L;
    public static final long EIGHT_BYTE = 0x1234567890ABCDEFL;
    public static final long POS_MAX = Long.MAX_VALUE;
    public static final long NEG_ONE = -1L;
    public static final long NEG_MAX = Long.MIN_VALUE;

    public static void main(String[] args) {
        System.out.printf("zero=%d%n", ZERO);
        System.out.printf("one_byte=%d%n", ONE_BYTE);
        System.out.printf("four_byte=%d%n", FOUR_BYTE);
        System.out.printf("five_byte=%d%n", FIVE_BYTE);
        System.out.printf("eight_byte=%d%n", EIGHT_BYTE);
        System.out.printf("pos_max=%d%n", POS_MAX);
        System.out.printf("neg_one=%d%n", NEG_ONE);
        System.out.printf("neg_max=%d%n", NEG_MAX);
    }
}
