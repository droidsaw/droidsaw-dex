// Minimal fixture that forces d8 to emit a canonical `invoke-custom`
// referencing a populated `call_site_ids` section. Uses a non-capturing
// lambda whose functional interface is `Supplier<String>`; under
// `--min-api 26` d8 preserves the LambdaMetafactory bootstrap rather than
// desugaring to a synthetic throw-stub.
//
// Exercises: invoke-custom opcode + call_site_id + method_handle_item +
// callsite_item encoded_array decoding in the decompile/emit paths.
import java.util.function.Supplier;

public class LambdaCallSite {
    public static void main(String[] args) {
        Supplier<String> s = () -> "x";
        System.out.println(s.get());
    }
}
