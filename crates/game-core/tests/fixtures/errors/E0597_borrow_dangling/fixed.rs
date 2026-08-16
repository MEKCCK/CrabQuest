fn main() {
    let s;
    {
        let t = String::from("hi");
        s = &t;
        println!("{}", s);
    }
}
