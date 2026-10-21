// canary role=pure
pub struct W(W);
pub fn run(w: W) {
    w.0.wrapping_add(w.0); //~ modular-arith
    w.0.saturating_sub(w.0); //~ modular-arith
}
