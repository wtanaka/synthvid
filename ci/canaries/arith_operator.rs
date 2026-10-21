// canary role=pure
pub struct W(W);
pub fn run(w: W) {
    let x = w.0 + w.0; //~ arith-operator
    let y = -w.0; //~ arith-operator
}
