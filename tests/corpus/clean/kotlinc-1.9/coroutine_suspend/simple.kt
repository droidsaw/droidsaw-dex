// suspend-fun state machine. Audit finding #10: state machine lives in the
// top-level wrapper method (NOT in the inner ContinuationImpl class).
// Strong discriminators (in DexSigInput<'a>):
//   - Continuation parameter on method signature
//   - inner class extends kotlin.coroutines.jvm.internal.ContinuationImpl
//   - body has `instanceof <inner> + label-bitmask preamble`
//   - body has `getfield label:I + tableswitch`
//   - default case throws IllegalStateException(
//       "call to 'resume' before 'invoke' with coroutine") — exact literal
//   - per-state ResultKt.throwOnFailure + IntrinsicsKt.getCOROUTINE_SUSPENDED
// OQ §5 RESOLVED IN AUDIT: recognizer fits DexSigInput<'a>; no follow-up split.

import kotlinx.coroutines.delay

suspend fun work(x: Int): Int {
    delay(1)
    val y = x + 1
    delay(1)
    return y
}
