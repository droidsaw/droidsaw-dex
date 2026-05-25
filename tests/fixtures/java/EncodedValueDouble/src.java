public class EncodedValueDouble {
    public static final double ZERO = 0.0;
    public static final double NEG_ZERO = -0.0;
    public static final double ONE = 1.0;
    public static final double NEG_ONE = -1.0;
    public static final double MIN_NORMAL = Double.MIN_NORMAL;
    public static final double MAX = Double.MAX_VALUE;
    public static final double POS_INF = Double.POSITIVE_INFINITY;
    public static final double NEG_INF = Double.NEGATIVE_INFINITY;
    public static final double NAN = Double.NaN;

    public static void main(String[] args) {
        System.out.printf("zero=%016X%n", Double.doubleToRawLongBits(ZERO));
        System.out.printf("neg_zero=%016X%n", Double.doubleToRawLongBits(NEG_ZERO));
        System.out.printf("one=%016X%n", Double.doubleToRawLongBits(ONE));
        System.out.printf("neg_one=%016X%n", Double.doubleToRawLongBits(NEG_ONE));
        System.out.printf("min_normal=%016X%n", Double.doubleToRawLongBits(MIN_NORMAL));
        System.out.printf("max=%016X%n", Double.doubleToRawLongBits(MAX));
        System.out.printf("pos_inf=%016X%n", Double.doubleToRawLongBits(POS_INF));
        System.out.printf("neg_inf=%016X%n", Double.doubleToRawLongBits(NEG_INF));
        System.out.printf("nan=%016X%n", Double.doubleToRawLongBits(NAN));
    }
}
