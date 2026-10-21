// canary role=pure
pub struct Config { pub a: Config, pub b: Config }
pub struct Wrapper(pub Config, pub Config);
pub enum Event { Move { x: Config, y: Config }, Pair(Config, Config), Quit }
pub enum Kind { A, B }
pub struct Tagged { pub kind: Kind, pub other: Kind }
pub struct Unit;
pub struct Braced {}
pub struct Tuple();
pub enum Shape { Nothing, Empty {} }
pub type Alias = Config;
mod inner { pub struct Deep { pub a: super::Config, pub b: super::Config } }
use inner::Deep as Moved;
pub fn lets(c: Config, w: Wrapper, e: Event) {
    let Config { a, .. } = c; //~ struct-rest-pattern
    let Wrapper(first, ..) = w; //~ struct-rest-pattern
    let Event::Move { x, .. } = e else { return }; //~ refutable-shortcut
}
pub fn arms(e: Event) {
    match e {
        Event::Move { x, .. } => (), //~ struct-rest-pattern
        Event::Pair(a, ..) => (), //~ struct-rest-pattern
        Event::Quit => (),
    }
}
pub fn conditionals(e: Event, o: Option<Config>, cs: Vec<Config>) {
    if let Event::Move { x, .. } = e {} //~ refutable-shortcut
    while let Event::Pair(a, ..) = e {} //~ refutable-shortcut
    if let Some(Config { a, .. }) = o {} //~ struct-rest-pattern
    if let Event::Pair(Config { a, .. }, ..) = e {} //~ struct-rest-pattern refutable-shortcut
    if let Event::Move { x, .. } = e && cs.is_empty() {} //~ refutable-shortcut
    for Config { a, .. } in cs {} //~ struct-rest-pattern
    let hit = matches!(e, Event::Move { .. }); //~ refutable-shortcut
}
pub fn closure() { let f = |Config { a, .. }: Config| a; } //~ struct-rest-pattern
pub fn param(Config { a, .. }: Config) {} //~ struct-rest-pattern
pub fn resolved(c: Alias, d: Moved) {
    let Alias { a, .. } = c; //~ struct-rest-pattern
    let Moved { a, .. } = d; //~ struct-rest-pattern
}
impl Config {
    pub fn first(self) -> Config { let Self { a, .. } = self; a } //~ struct-rest-pattern
}
macro_rules! take { ($c:expr) => { let Config { a, .. } = $c; }; } //~ struct-rest-pattern unparsed-macro
pub fn invoked(c: Config) {
    wrap! { let Config { a, .. } = c; } //~ struct-rest-pattern
}
pub fn owned_by_catch_all(t: Tagged) {
    match t {
        Tagged { kind: Kind::A, .. } => (), //~ catch-all-arm
        Tagged { kind: Kind::B, .. } => (), //~ catch-all-arm
    }
}
pub fn external(m: syn::MetaList, r: std::ops::Range<Config>, u: Unknown) {
    let syn::MetaList { path, .. } = m;
    let std::ops::Range { start, .. } = r;
    let Unknown { x, .. } = u;
}
pub fn not_struct_fields(s: &[Config], t: (Config, Config)) {
    let [first, ..] = s;
    let (head, ..) = t;
}
pub fn explicit(c: Config, w: Wrapper, e: Event) {
    let Config { a, b: _ } = c;
    let Wrapper(first, _) = w;
    match e {
        Event::Move { x, y: _ } => (),
        Event::Pair(a, _) => (),
        Event::Quit => (),
    }
}
pub fn fieldless(u: Unit, b: Braced, t: Tuple, s: Shape) {
    let Unit { .. } = u;
    let Braced { .. } = b;
    let Tuple(..) = t;
    match s {
        Shape::Nothing { .. } => (),
        Shape::Empty { .. } => (),
    }
}
