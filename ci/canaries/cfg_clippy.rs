// canary role=pure
#[cfg(clippy)] //~ cfg-clippy
pub fn run() {}
#[cfg_attr(clippy, derive(Debug))] //~ cfg-clippy
pub struct S;
