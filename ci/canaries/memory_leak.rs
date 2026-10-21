// canary role=pure
use std::mem::ManuallyDrop; //~ memory-leak
use std::boxed::Box as B;
use std::ffi::OsString;
use std::mem as m;
pub struct Item;
pub struct Held(ManuallyDrop<Item>); //~ memory-leak
pub fn boxed(item: Box<Item>) -> &'static mut Item { Box::leak(item) } //~ memory-leak
pub fn aliased(item: Box<Item>) -> &'static mut Item { B::leak(item) } //~ memory-leak
pub fn full(item: Box<Item>) -> &'static mut Item { std::boxed::Box::leak(item) } //~ memory-leak
pub fn alloc_path(item: Box<Item>) -> &'static mut Item { alloc::boxed::Box::leak(item) } //~ memory-leak
pub fn counted(item: OsString) { OsString::leak(item); } //~ memory-leak
pub fn shared(item: std::path::PathBuf) { std::path::PathBuf::leak(item); } //~ memory-leak
pub fn vector(items: Vec<Item>) { Vec::leak(items); } //~ memory-leak
pub fn text() { String::leak(String::new()); } //~ memory-leak
pub fn method(items: Vec<Item>) { items.leak(); } //~ memory-leak
pub fn make(item: Item) -> Held { Held(m::ManuallyDrop::new(item)) } //~ memory-leak
pub fn core_path(item: Item) { core::mem::ManuallyDrop::new(item); } //~ memory-leak
pub fn dropped(held: &mut Held) { m::ManuallyDrop::take(&mut held.0); } //~ memory-leak
pub fn owned(item: Box<Item>) { drop(item); }
pub fn forgotten(item: Item) { std::mem::forget(item); }
pub fn raw(item: Box<Item>) { Box::into_raw(item); }
pub fn leak_with_argument(detector: Detector, item: Item) { detector.leak(item); }
pub struct Detector;
mod mine {
    pub struct Box;
    impl Box { pub fn leak() {} }
    pub struct ManuallyDrop;
    pub fn go() { Box::leak(); ManuallyDrop::new(); }
}
