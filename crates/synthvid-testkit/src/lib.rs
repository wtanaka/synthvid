//! Test support for the synthvid crates.
//!
//! A check is a function returning `Result<(), Fail>`. It reaches a value
//! through `?` and [`OrFail::or_fail`] instead of `unwrap`, and states its
//! expectations with [`ensure!`] and [`ensure_eq!`] instead of `assert!`. The
//! `#[test]` function that runs it asserts the outcome once:
//!
//! ```
//! use synthvid_testkit::{ensure_eq, Fail, OrFail};
//!
//! fn check_parse() -> Result<(), Fail> {
//!     let value: u8 = "7".parse().or_fail()?;
//!     ensure_eq!(value, 7);
//!     Ok(())
//! }
//!
//! assert_eq!(check_parse(), Ok(()));
//! ```
//!
//! A failed check names the line that failed, so the assertion in the test
//! reports where and why without the check ever aborting.

mod fail;
mod macros;

pub use fail::{Fail, OrFail};
