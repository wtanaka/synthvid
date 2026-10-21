// canary role=pure
pub fn run() {
    let _ = run(); //~ discarded-value
    _ = run(); //~ discarded-value
}
