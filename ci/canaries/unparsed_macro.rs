// canary role=pure
macro_rules! m { () => { x.unwrap() } } //~ abort-unwrap unparsed-macro
