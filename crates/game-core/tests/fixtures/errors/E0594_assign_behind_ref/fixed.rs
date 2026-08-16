fn main() {
    let mut n = 1;
    f(&mut n);
    println!("{}", n);
}

fn f(x: &mut i32) {
    *x = 5;
}
