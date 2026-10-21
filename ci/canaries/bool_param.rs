// canary role=pure
pub fn run(flag: bool) {} //~ bool-param
pub fn walk() { let _f = |on: bool| (); } //~ bool-param unused-underscore-binding
