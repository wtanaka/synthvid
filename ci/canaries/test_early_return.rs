// canary role=pure
pub struct E;
#[test]
fn bare_return() {
    return; //~ test-early-return
    assert!(true);
}
#[test]
fn return_ok_unit() -> Result<(), E> {
    return Ok(()); //~ test-early-return
    assert!(true);
}
#[test]
fn return_ok_value() -> Result<E, E> {
    return Ok(E); //~ test-early-return
    assert!(true);
}
#[test]
fn return_in_if() {
    if go() {
        return; //~ test-early-return
    }
    assert!(true);
}
#[test]
fn return_in_else_branch() {
    if go() {
        go();
    } else {
        return; //~ test-early-return
    }
    assert!(true);
}
#[test]
fn return_in_match_arm() {
    match go() {
        1 => return, //~ test-early-return
        _ => go(),
    }
    assert!(true);
}
#[test]
fn return_in_loop() {
    loop {
        return; //~ test-early-return
    }
    assert!(true);
}
#[test]
fn return_in_block() {
    {
        return; //~ test-early-return
    }
    assert!(true);
}
#[test]
fn let_else_returns() {
    let Some(x) = go() else { //~ test-early-return
        return;
    };
    assert!(true);
}
#[test]
fn let_else_returns_ok() -> Result<(), E> {
    let Some(x) = go() else { //~ test-early-return
        return Ok(());
    };
    assert!(true);
    Ok(())
}
#[test]
fn let_else_returns_after_branch() {
    let Some(x) = go() else { //~ test-early-return
        if go() {
            return;
        }
        go();
        return;
    };
    assert!(true);
}
#[test]
fn return_err_fails_the_test() -> Result<(), E> {
    if go() {
        return Err(E);
    }
    assert!(true);
    Ok(())
}
#[test]
fn return_err_converted() -> Result<(), E> {
    if go() {
        return Err(x.into());
    }
    assert!(true);
    Ok(())
}
#[test]
fn return_err_full_path() -> Result<(), E> {
    if go() {
        return core::result::Result::Err(E);
    }
    if go() {
        return Result::Err(E);
    }
    assert!(true);
    Ok(())
}
#[test]
fn let_else_returns_err() -> Result<(), E> {
    let Some(x) = go() else {
        return Err(E);
    };
    assert!(true);
    Ok(())
}
#[test]
fn return_other_expr_is_flagged() -> Result<(), E> {
    let outcome = Ok(());
    return outcome; //~ test-early-return
    assert!(true);
}
#[test]
fn return_err_then_bare_return() -> Result<(), E> {
    if go() {
        return Err(E);
    }
    return Ok(()); //~ test-early-return
    assert!(true);
}
#[tokio::test]
async fn async_test_returns() {
    return; //~ test-early-return
    assert!(true);
}
#[test]
fn closure_does_not_count_but_own_return_does() {
    let f = || {
        return;
    };
    return; //~ test-early-return
    assert!(true);
}
#[test]
fn each_let_else_before_the_assertion_is_reported() {
    let Some(a) = go() else { //~ test-early-return
        return;
    };
    let Some(b) = go() else { //~ test-early-return
        return;
    };
    assert!(true);
}
mod nested {
    #[test]
    fn in_a_module() {
        return; //~ test-early-return
        assert!(true);
    }
}
macro_rules! suite { //~ unparsed-macro
    () => {
        #[test]
        fn in_macro_rules() {
            return; //~ test-early-return
            assert!(true);
        }
    };
}
suite! {
    #[test]
    fn in_macro_body() {
        return; //~ test-early-return
        assert!(true);
    }
}
weird!(=> ;; #[test] fn token_scan() { //~ unparsed-macro
    return; //~ test-early-return
    assert!(true);
});
weird!(=> ;; #[test] fn token_scan_closure() { //~ unparsed-macro
    let f = || { return; };
    assert!(true);
});
weird!(=> ;; #[test] fn token_scan_err() { //~ unparsed-macro
    if go() { return Err(E); }
    if go() { return core::result::Result::Err(E); }
    assert!(true);
});
weird!(=> ;; #[test] fn token_scan_ok_flagged() { //~ unparsed-macro
    return Ok(()); //~ test-early-return
    assert!(true);
});
