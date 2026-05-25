// Wide-type bitwise ops: long shifts, masks, unsigned-right-shift.
// Avoids long-comparison loops (cmp-long + if-*z fusion is a known gap).
public class BitsWide {

    // Rotate left by k bits (long)
    static long rotateLeft(long v, int k) {
        return (v << k) | (v >>> (64 - k));
    }

    // Extract byte at position (0 = lowest)
    static int extractByte(long v, int pos) {
        return (int)((v >>> (pos * 8)) & 0xFFL);
    }

    // Mask and combine: set low byte, preserve rest
    static long setLowByte(long v, int b) {
        return (v & ~0xFFL) | (b & 0xFFL);
    }

    // XOR fold: xor high and low 32 bits
    static long xorFold(long v) {
        return (v >>> 32) ^ v;
    }

    // Isolate nibbles (exercises inline precedence across & and <<)
    static long isolateNibbles(long v) {
        long lo = v & 0x0FL;
        long hi = (v >>> 4) & 0x0FL;
        return (hi << 8) | lo;
    }

    // Kernighan popcount on long (exercises cmp-long + if-*z fusion)
    static int popcountLong(long n) {
        int c = 0;
        while (n != 0L) {
            n = n & (n - 1L);
            c++;
        }
        return c;
    }

    public static void main(String[] args) {
        System.out.println(rotateLeft(1L, 3));               // 8
        System.out.println(extractByte(0xDEADBEEFL, 1));     // 190
        System.out.println(setLowByte(0xFF00L, 0xAB));       // 65451
        System.out.println(xorFold(0x00000001FFFFFFFFL));    // 8589934590
        System.out.println(popcountLong(0xFFFFL));            // 16
        System.out.println(isolateNibbles(0xA5L));           // 2565
    }
}
