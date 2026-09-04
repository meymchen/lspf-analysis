fn add(a: u32, b: u32) -> u32 {
    a + b
}

fn describe(value: u32) -> &'static str {
    if value == 0 { "zero" } else { "some" }
}
