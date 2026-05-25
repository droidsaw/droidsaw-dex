// Minimal fixture that forces d8 to emit `invoke-polymorphic` for a
// `MethodHandle.invokeExact` call. Requires `--min-api 26`; under d8's
// default the call is desugared to a synthetic throw-stub.
//
// Exercises: invoke-polymorphic opcode + proto_id (the per-call-site
// signature) decoding in the decompile path.
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;

public class MethodHandleInvoke {
    public static String hello() {
        return "hi";
    }

    public static void main(String[] args) throws Throwable {
        MethodHandle mh = MethodHandles.lookup().findStatic(
            MethodHandleInvoke.class,
            "hello",
            MethodType.methodType(String.class));
        String s = (String) mh.invokeExact();
        System.out.println(s);
    }
}
