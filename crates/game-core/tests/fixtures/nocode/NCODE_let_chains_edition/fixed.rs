fn main() {
    let x = Some(5);
    if let Some(n) = x {
        if n > 3 {
            println!("{}", n);
        }
    }
}
