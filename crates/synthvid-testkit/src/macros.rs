//! Assertion-shaped macros that return a [`Fail`](crate::Fail) instead of panicking.

/// Fails the enclosing check unless `condition` holds.
#[macro_export]
macro_rules! ensure {
    ($condition:expr_2021 $(,)?) => {
        if !$condition {
            return Err($crate::Fail::here(format_args!(
                "expected {}",
                stringify!($condition)
            )));
        }
    };
    ($condition:expr_2021, $($note:tt)+) => {
        if !$condition {
            return Err($crate::Fail::here(format_args!(
                "expected {}: {}",
                stringify!($condition),
                format_args!($($note)+)
            )));
        }
    };
}

/// Fails the enclosing check unless both sides are equal.
#[macro_export]
macro_rules! ensure_eq {
    ($left:expr_2021, $right:expr_2021 $(,)?) => {
        $crate::ensure_eq!($left, $right, "{} == {}", stringify!($left), stringify!($right))
    };
    ($left:expr_2021, $right:expr_2021, $($note:tt)+) => {
        match (&$left, &$right) {
            (left, right) => {
                if *left != *right {
                    return Err($crate::Fail::here(format_args!(
                        "{}\n  left: {:?}\n right: {:?}",
                        format_args!($($note)+),
                        left,
                        right
                    )));
                }
            }
        }
    };
}

/// Fails the enclosing check if both sides are equal.
#[macro_export]
macro_rules! ensure_ne {
    ($left:expr_2021, $right:expr_2021 $(,)?) => {
        $crate::ensure_ne!($left, $right, "{} != {}", stringify!($left), stringify!($right))
    };
    ($left:expr_2021, $right:expr_2021, $($note:tt)+) => {
        match (&$left, &$right) {
            (left, right) => {
                if *left == *right {
                    return Err($crate::Fail::here(format_args!(
                        "{}\n  both: {:?}",
                        format_args!($($note)+),
                        left
                    )));
                }
            }
        }
    };
}
