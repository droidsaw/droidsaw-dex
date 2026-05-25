public class EncodedValueShort {
    public static final short ZERO = 0;
    public static final short ONE_BYTE_POS = 127;
    public static final short ONE_BYTE_NEG = -128;
    public static final short TWO_BYTE_POS = 0x1234;
    public static final short POS_MAX = Short.MAX_VALUE;
    public static final short NEG_MAX = Short.MIN_VALUE;

    public static void main(String[] args) {
        System.out.printf("zero=%d%n", (int) ZERO);
        System.out.printf("one_byte_pos=%d%n", (int) ONE_BYTE_POS);
        System.out.printf("one_byte_neg=%d%n", (int) ONE_BYTE_NEG);
        System.out.printf("two_byte_pos=%d%n", (int) TWO_BYTE_POS);
        System.out.printf("pos_max=%d%n", (int) POS_MAX);
        System.out.printf("neg_max=%d%n", (int) NEG_MAX);
    }
}
