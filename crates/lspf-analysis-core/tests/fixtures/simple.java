interface Calculator {
    int add(int a, int b);
}

public class Simple implements Calculator {
    public int base;
    private int offset;

    public int add(int a, int b) {
        return a + b;
    }

    private int classify(int a, int b, int c) {
        int total = 0;
        if (a > 1) {
            if (b > 2) {
                if (c > 3) {
                    total = a + b + c;
                }
            }
        }
        return total;
    }
}
