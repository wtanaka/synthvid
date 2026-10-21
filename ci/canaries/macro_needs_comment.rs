// canary role=pure
proptest! { #[test] fn p(x in 0..3u8) { prop_assert!(x < 3); } } //~ macro-needs-comment
