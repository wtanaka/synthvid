// canary role=pure
pub fn run() {
    panic!("x"); //~ abort-panic
    todo!(); //~ abort-panic
    std::process::exit(1); //~ abort-panic ambient-input
}
