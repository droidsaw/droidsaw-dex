public class EncodedValueByte {
    public static final byte ZERO = 0;
    public static final byte POS_MAX = Byte.MAX_VALUE;
    public static final byte NEG_ONE = -1;
    public static final byte NEG_MAX = Byte.MIN_VALUE;

    public static void main(String[] args) {
        System.out.printf("zero=%d%n", (int) ZERO);
        System.out.printf("pos_max=%d%n", (int) POS_MAX);
        System.out.printf("neg_one=%d%n", (int) NEG_ONE);
        System.out.printf("neg_max=%d%n", (int) NEG_MAX);
    }
}
