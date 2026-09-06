// Small functions for metric and health regression coverage.
int add(int a, int b) {
    return a + b;
}

int absolute(int value) {
    if (value < 0) {
        return -value;
    }
    return value;
}
