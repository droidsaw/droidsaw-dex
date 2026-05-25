// DJB-inspired: bitwise ops, Kernighan popcount, XOR cipher.
public class Bits {

    // Kernighan's popcount: clears lowest set bit each iteration
    static int popcount(int n) {
        int c = 0;
        while (n != 0) {
            n = n & (n - 1);
            c++;
        }
        return c;
    }

    // Position of highest set bit (floor log2 for n >= 1)
    static int highBit(int n) {
        int p = 0;
        while (n > 1) {
            n = n >>> 1;
            p++;
        }
        return p;
    }

    // XOR each byte with key (int key avoids byte-literal cast in caller)
    static byte[] xorBytes(byte[] data, int key) {
        byte[] out = new byte[data.length];
        for (int i = 0; i < data.length; i++) {
            out[i] = (byte)(data[i] ^ key);
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(popcount(255));   // 8
        System.out.println(popcount(0));     // 0
        System.out.println(highBit(4));      // 2
        byte[] msg = new byte[3];
        msg[0] = (byte)'H';
        msg[1] = (byte)'e';
        msg[2] = (byte)'l';
        byte[] enc = xorBytes(msg, 32);
        System.out.println(new String(enc)); // hEL
    }
}
