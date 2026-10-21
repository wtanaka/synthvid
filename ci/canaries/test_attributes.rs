// canary role=pure
#[test]
#[ignore] //~ test-attributes
fn t() { assert!(true); }
#[test]
#[should_panic] //~ test-attributes
fn u() { assert!(true); }
