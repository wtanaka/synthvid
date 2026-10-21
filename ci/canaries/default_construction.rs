// canary role=pure
pub struct Item;
#[derive(Debug, Default)] //~ default-construction
pub struct Plain { pub a: Item }
#[derive(Default)] //~ default-construction
pub struct Tuple(pub Item);
#[derive(Default)] //~ default-construction
pub struct Unit;
#[derive(Default)] //~ default-construction
pub struct Generic<T> { pub value: T }
#[derive(Default)] //~ default-construction
pub union Raw { pub a: Item }
#[cfg_attr(feature = "x", derive(Default))] //~ default-construction
pub struct Gated { pub a: Item }
#[derive(std::default::Default)] //~ default-construction
pub struct Qualified { pub a: Item }
#[derive(Default)]
pub enum Mode { #[default] Fast, Slow }
#[derive(Clone, Debug)]
pub struct Quiet { pub a: Item }
macro_rules! make { () => { #[derive(Default)] pub struct Made { pub a: Item } }; } //~ default-construction unparsed-macro
wrap! {
    #[derive(Default)] //~ default-construction
    pub struct Wrapped { pub a: Item }
}
pub struct Pair { pub a: Item, pub b: Item }
impl Default for Pair {
    fn default() -> Self { Pair { a: Item, b: Item } }
}
impl Pair {
    pub fn make(a: Item) -> Self { Self { a, ..Default::default() } } //~ default-construction
}
pub fn updates(a: Item, old: Pair) -> Pair {
    let first = Pair { a, ..Default::default() }; //~ default-construction
    let second = Pair { a: Item, ..Pair::default() }; //~ default-construction
    let third = Pair { a: Item, ..<Pair as Default>::default() }; //~ default-construction
    let fourth = Pair { a: Item, ..core::default::Default::default() }; //~ default-construction
    let fifth = Pair { a: Item, ..old };
    let sixth = Pair { a: Item, ..old.clone() };
    let seventh: Pair = Default::default();
    let eighth = Pair::default();
    let ninth = syn::MetaList { path: Item, ..Default::default() };
    let tenth = Unknown { a: Item, ..Default::default() };
    first
}
