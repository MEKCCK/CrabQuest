fn main() {
    let mut v = Vec::new();
    v.push(1);
    fill_vec(v);
}

fn fill_vec(vec: Vec<i32>) -> Vec<i32> {
    vec.push(22);
    vec.push(44);
    vec
}
