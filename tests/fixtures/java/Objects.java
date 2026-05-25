// Exercise constructors, instance fields, manhattan distance, instance methods.
public class Objects {

    int x;
    int y;

    Objects(int x, int y) {
        this.x = x;
        this.y = y;
    }

    // Manhattan (L1) distance
    int dist(Objects other) {
        int dx = this.x - other.x;
        int dy = this.y - other.y;
        if (dx < 0) {
            dx = -dx;
        }
        if (dy < 0) {
            dy = -dy;
        }
        return dx + dy;
    }

    boolean isOrigin() {
        return this.x == 0 && this.y == 0;
    }

    public static void main(String[] args) {
        Objects a = new Objects(0, 0);
        Objects b = new Objects(2, 3);
        System.out.println(a.dist(b));      // 5
        System.out.println(a.isOrigin());   // true
        System.out.println(b.isOrigin());   // false
        System.out.println(b.dist(a));      // 5
    }
}
