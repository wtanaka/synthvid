// canary role=pure
pub struct Item;
pub enum Kind { A, B }
pub fn fallible() -> Result<Item, Item> { Ok(Item) }
pub fn consume<T>(value: T) { drop(value); }
pub fn work() {}
pub fn flagged(result: Result<Item, Item>) {
    match result {
        Ok(item) => consume(item),
        Err(_) => (), //~ swallowed-error
    }
    match result { Ok(item) => consume(item), Err(..) => {} } //~ swallowed-error
    match result {
        Ok(item) => consume(item),
        Err(_reason) => { () } //~ swallowed-error
    }
    match result { Ok(item) => consume(item), Result::Err(_) => {} } //~ swallowed-error
    match result { Ok(item) => consume(item), std::result::Result::Err(_) => () } //~ swallowed-error
    match result { Ok(item) => consume(item), core::result::Result::Err(_) => () } //~ swallowed-error
    if let Ok(item) = fallible() { consume(item); } //~ swallowed-error
    while let Ok(item) = fallible() { consume(item); } //~ swallowed-error
    if let Ok(item) = fallible() && fallible().is_ok() { consume(item); } //~ swallowed-error
    if fallible().is_ok() && let Ok(item) = fallible() { consume(item); } //~ swallowed-error
    while let Ok(item) = fallible() && fallible().is_ok() { consume(item); } //~ swallowed-error
}
pub fn handled(result: Result<Item, Item>, option: Option<Item>) {
    match result { Ok(item) => consume(item), Err(reason) => consume(reason) }
    match result { Ok(item) => consume(item), Err(_) => work() }
    match result { Ok(item) => consume(item), Err(_) => { work(); } }
    match result { Ok(item) => consume(item), Err(named) => () }
    if let Ok(item) = fallible() { consume(item); } else { work(); }
    if let Some(item) = option { consume(item); }
    if let Ok(item) = fallible() && fallible().is_ok() { consume(item); } else { work(); }
    match option { Some(item) => consume(item), None => () }
}
pub fn overlap(result: Result<Kind, Kind>) {
    match result {
        Ok(kind) => consume(kind),
        Err(Kind::A) => work(),
        Err(_) => (), //~ catch-all-arm
    }
    if let Ok(Kind::A) = result { work(); } //~ refutable-shortcut
    if let Ok(Kind::A) = result && fallible().is_ok() { work(); } //~ refutable-shortcut
}
mod mine {
    pub struct Item;
    pub enum Outcome { Err(Item), Fine }
    use Outcome::*;
    pub fn go(outcome: Outcome) {
        match outcome { Err(_) => (), Fine => () }
    }
}
