// canary role=pure
pub fn prod() { assert!(true); } //~ abort-assert
#[cfg(test)]
mod tests {
    pub fn helper() { assert!(true); }
    #[test]
    fn holds() { assert_eq!(1, 1); }
}
#[cfg(all(test, feature = "x"))]
mod more { pub fn helper() { assert_ne!(1, 2); } }
#[cfg(any(test, unix))]
mod maybe { pub fn helper() { assert!(true); } } //~ abort-assert
