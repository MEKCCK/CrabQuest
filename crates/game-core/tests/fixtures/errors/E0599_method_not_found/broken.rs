struct Rectangle {
    width: u32,
    height: u32,
}

trait Area {
    fn area(&self) -> u32;
}

fn main() {
    let r = Rectangle { width: 4, height: 5 };
    println!("area: {}", r.area());
}
