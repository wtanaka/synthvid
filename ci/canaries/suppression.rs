// canary role=pure
#[allow(dead_code)] //~ suppression
pub fn run() {}
#[cfg_attr(feature = "x", warn(unused))] //~ suppression
pub fn walk() {}
#[expect(dead_code)] //~ suppression
pub fn jog() {}
