// canary role=pure
pub fn run() {
    std::env::args(); //~ ambient-input
    println!("x"); //~ ambient-input
}
