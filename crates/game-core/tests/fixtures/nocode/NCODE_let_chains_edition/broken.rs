fn main() {
    let x = Some(5);
    if let Some(n) = x && n > 3 {
        println!("{}", n);
    }
}
