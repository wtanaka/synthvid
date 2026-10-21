// canary role=pure
pub struct W(Option<W>);
pub fn run(w: W) {
    w.0.unwrap_or(w); //~ silent-fallback
    w.0.map_or(w, w); //~ silent-fallback
    w.0.ok(); //~ silent-fallback
}
