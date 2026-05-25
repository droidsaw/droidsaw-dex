// Covers Java 8 invoke-dynamic lowering for lambdas. javac + d8 lower
// `() -> ...` via LambdaMetafactory into synthetic static methods plus
// an invoke-custom bootstrap. Decompile should restore the original
// lambda syntax; a capturing lambda additionally needs its captured
// locals threaded through the synthetic site's constant args.
import java.util.function.Function;

public class Lambdas {
    public static void main(String[] args) {
        Runnable r = () -> System.out.println("hello");
        r.run();

        Function<Integer, Integer> sq = x -> x * x;
        System.out.println(sq.apply(7));

        int c = 10;
        Function<Integer, Integer> addC = x -> x + c;
        System.out.println(addC.apply(5));
    }
}
