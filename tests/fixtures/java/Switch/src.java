// Dense integer switch with a default arm. d8 will emit a
// packed-switch payload; tests the structuring pass's switch
// recovery.
public class Switch {
    static String dayName(int d) {
        switch (d) {
            case 1: return "Mon";
            case 2: return "Tue";
            case 3: return "Wed";
            case 4: return "Thu";
            case 5: return "Fri";
            default: return "weekend";
        }
    }

    public static void main(String[] args) {
        System.out.println(dayName(1));
        System.out.println(dayName(3));
        System.out.println(dayName(6));
    }
}
