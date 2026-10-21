// canary role=pure
pub struct W(Option<W>);
pub fn run(w: W) {
    w.0.unwrap(); //~ abort-unwrap
    w.0.expect("x"); //~ abort-unwrap
    Option::unwrap(w.0); //~ abort-unwrap
}
