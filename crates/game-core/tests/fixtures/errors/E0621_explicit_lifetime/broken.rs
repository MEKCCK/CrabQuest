fn foo<'a>(x: &'a i32, y: &i32) -> &'a i32 {
    if x > y {
        x
    } else {
        y
    }
}

fn main() {
    let a = 5;
    let b = 3;
    println!("{}", foo(&a, &b));
}
