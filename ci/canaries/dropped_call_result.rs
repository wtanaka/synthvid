// canary role=pure
use std::mem;
use std::mem::drop as release;
pub struct Item;
pub struct Pair { pub left: Item, pub right: Item }
pub fn fallible() -> Result<Item, Item> { Ok(Item) }
pub fn flagged(items: Vec<Item>, item: Item) -> Result<(), Item> {
    drop(fallible()); //~ dropped-call-result
    std::mem::drop(fallible()); //~ dropped-call-result
    core::mem::drop(fallible()); //~ dropped-call-result
    mem::drop(fallible()); //~ dropped-call-result
    release(fallible()); //~ dropped-call-result
    drop(fallible()?); //~ dropped-call-result
    drop(format!("x")); //~ dropped-call-result
    drop(items.len()); //~ dropped-call-result
    drop((fallible())); //~ dropped-call-result
    Ok(())
}
pub fn allowed(pair: Pair, item: Item) {
    let guard = fallible();
    drop(guard);
    let _guard = fallible();
    drop(_guard);
    drop(item);
    drop(pair.left);
    drop(&pair.right);
    drop((Item, Item));
}
mod mine {
    pub fn drop<T>(value: T) { let moved = value; }
    pub fn go() { drop(super::fallible()); }
}
