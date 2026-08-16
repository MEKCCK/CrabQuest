fn main() {
    let x = String::from("hi");
    let r = &x;
    let f = move || {
        let _ = x;
    };
    let _ = (r, f);
}
