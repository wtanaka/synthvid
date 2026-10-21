// canary role=pure
pub fn run() {
    assert!(true); //~ abort-assert
    debug_assert_eq!(1, 1); //~ abort-assert
}
