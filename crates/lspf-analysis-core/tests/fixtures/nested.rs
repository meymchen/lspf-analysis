fn classify(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {
    let mut total = 0;
    if a > 1 {
        if b > 2 {
            if c > 3 {
                if d > 4 {
                    let step = a + b + c + d + e;
                    total = total + step;
                }
            }
        }
    }
    for index in 0..a {
        while b > index {
            if c > index {
                total = total + index;
            }
        }
    }
    total
}
