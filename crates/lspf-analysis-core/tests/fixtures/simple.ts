function add(a: number, b: number): number {
    return a + b;
}

function classify(a: number, b: number, c: number): number {
    let total = 0;
    if (a > 1) {
        if (b > 2) {
            if (c > 3) {
                total = a + b + c;
            }
        }
    }
    return total;
}
