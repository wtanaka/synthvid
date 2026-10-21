// canary role=pure
pub struct Item;
pub struct Pair { pub left: Item, pub right: Item }
pub fn fallible() -> Result<Item, Item> { Ok(Item) }
pub fn consume<T>(value: T) { drop(value); }
pub fn work() {}
pub fn flagged(pair: Pair, opt: Option<Item>) {
    let _result = fallible(); //~ unused-underscore-binding
    let _typed: Result<Item, Item> = fallible(); //~ unused-underscore-binding
    let (kept, _second) = (Item, fallible()); //~ unused-underscore-binding
    let Pair { left, right: _right } = pair; //~ unused-underscore-binding
    let [_first, ..] = [Item, Item]; //~ unused-underscore-binding
    let Some(_value) = opt else { return }; //~ unused-underscore-binding
    let _guard = fallible(); //~ unused-underscore-binding
}
pub fn reads(items: Vec<Item>, opt: Option<Item>) {
    let _a = fallible();
    consume(_a);
    let _b = fallible();
    let reader = || _b;
    let _c = Item;
    let text = format!("{_c:?}");
    let _d = fallible();
    use_it!(_d);
    let _guard = fallible();
    work();
    drop(_guard);
    let _e = fallible();
    { consume(_e); }
    let _shadow = fallible();
    let _shadow = Item;
    consume(_shadow);
}
pub fn not_covered(_param: Item, items: Vec<Item>, opt: Option<Item>) {
    let closure = |_x: Item| ();
    for _item in items { work(); }
    match opt { Some(_inner) => work(), None => work() }
    if let Some(_other) = opt { work(); }
}
macro_rules! bind { () => { let _hidden = fallible(); }; } //~ unused-underscore-binding unparsed-macro
pub fn invoked() {
    wrap! { let _inside = fallible(); } //~ unused-underscore-binding
}
