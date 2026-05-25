// adapted from R8 test src/test/examples/classmerging/ExceptionTest.java (Apache-2)
// Exercises a 4-class exception subclass chain plus an ordered catch
// chain where catches are listed most-specific-first. Two throw sites
// (each `throws` a leaf type) keep all catch arms reachable per JLS;
// the runtime's catch dispatch must resolve to the first arm that
// matches the dynamic type, not a later (broader) arm.
public class ExceptionHierarchy {
    public static void main(String[] args) {
        runOnce("B");
        runOnce("2");
    }

    private static void runOnce(String which) {
        try {
            if (which.equals("B")) {
                throwExceptionB();
            } else {
                throwException2();
            }
        } catch (ExceptionB exception) {
            System.out.println("Caught ExceptionB: " + exception.getMessage());
        } catch (ExceptionA exception) {
            System.out.println("Caught ExceptionA: " + exception.getMessage());
        } catch (Exception2 exception) {
            System.out.println("Caught Exception2: " + exception.getMessage());
        } catch (Exception1 exception) {
            System.out.println("Caught Exception1: " + exception.getMessage());
        }
    }

    private static void throwExceptionB() throws ExceptionB {
        throw new ExceptionB("ouch-B");
    }

    private static void throwException2() throws Exception2 {
        throw new Exception2("ouch-2");
    }

    public static class ExceptionA extends Exception {
        public ExceptionA(String message) { super(message); }
    }

    public static class ExceptionB extends ExceptionA {
        public ExceptionB(String message) { super(message); }
    }

    public static class Exception1 extends Exception {
        public Exception1(String message) { super(message); }
    }

    public static class Exception2 extends Exception1 {
        public Exception2(String message) { super(message); }
    }
}
