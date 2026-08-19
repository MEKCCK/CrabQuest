fn main() {
    let mut n = 1;
    f(&n);
    println!("{}", n);
}

fn f(x: &i32) {
    *x = 5;
}
