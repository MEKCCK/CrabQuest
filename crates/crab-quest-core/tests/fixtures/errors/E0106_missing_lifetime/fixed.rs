fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() {
        x
    } else {
        y
    }
}

fn main() {
    let a = String::from("aaa");
    let b = String::from("bb");
    println!("{}", longest(&a, &b));
}
