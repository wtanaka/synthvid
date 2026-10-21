// canary role=pure
pub enum K { A, B }
pub fn run(k: K) {
    if let K::A = k {} //~ refutable-shortcut
    let b = matches!(k, K::B); //~ refutable-shortcut
}
