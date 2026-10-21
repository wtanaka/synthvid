// canary role=pure
use std::collections::HashMap; //~ unordered-collection
use std::collections as c; // alias
pub fn run() { let _s: c::HashSet<()> = c::HashSet::new(); } //~ unordered-collection unused-underscore-binding
