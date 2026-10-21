// canary role=pure
pub struct W(W);
pub fn run(w: W) {
    let x = w.0 as usize; //~ numeric-cast
    let y = w.0 as _; //~ numeric-cast
}
