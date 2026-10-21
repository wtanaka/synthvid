// canary role=pure
pub enum K { A, B }
pub fn run(k: K) {
    match k {
        K::A => (),
        _ => (), //~ catch-all-arm
    }
    match k {
        K::A => (),
        other => other, //~ catch-all-arm
    }
}
