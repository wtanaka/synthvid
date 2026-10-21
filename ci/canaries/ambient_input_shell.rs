// canary role=shell
use std::fs as f; //~ ambient-input
pub fn run() { f::write("x", ""); } //~ ambient-input
