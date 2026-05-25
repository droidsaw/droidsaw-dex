public class EncodedValueFloat {
    public static final float ZERO = 0.0f;
    public static final float NEG_ZERO = -0.0f;
    public static final float ONE = 1.0f;
    public static final float NEG_ONE = -1.0f;
    public static final float MIN_NORMAL = Float.MIN_NORMAL;
    public static final float MAX = Float.MAX_VALUE;
    public static final float POS_INF = Float.POSITIVE_INFINITY;
    public static final float NEG_INF = Float.NEGATIVE_INFINITY;
    public static final float NAN = Float.NaN;

    public static void main(String[] args) {
        System.out.printf("zero=%08X%n", Float.floatToRawIntBits(ZERO));
        System.out.printf("neg_zero=%08X%n", Float.floatToRawIntBits(NEG_ZERO));
        System.out.printf("one=%08X%n", Float.floatToRawIntBits(ONE));
        System.out.printf("neg_one=%08X%n", Float.floatToRawIntBits(NEG_ONE));
        System.out.printf("min_normal=%08X%n", Float.floatToRawIntBits(MIN_NORMAL));
        System.out.printf("max=%08X%n", Float.floatToRawIntBits(MAX));
        System.out.printf("pos_inf=%08X%n", Float.floatToRawIntBits(POS_INF));
        System.out.printf("neg_inf=%08X%n", Float.floatToRawIntBits(NEG_INF));
        System.out.printf("nan=%08X%n", Float.floatToRawIntBits(NAN));
    }
}
