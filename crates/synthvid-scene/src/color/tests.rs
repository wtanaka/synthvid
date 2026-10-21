use super::{blend_rgb, Alpha, Channel, Rgb8};
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

/// `blend_rgb` returns `Option` only because `arithmetic_side_effects`
/// forces `checked_*`; the arithmetic itself cannot overflow. Since
/// `alpha + inv_alpha == 255`, the weighted sum is at most `255 * 255`, so
/// every intermediate fits in `u16`, let alone the `u32` it uses.
///
/// The whole input space is `256^3`, so that claim is checked exhaustively
/// rather than argued. The reference is computed in `u64` with a different
/// shape, so it does not inherit a mistake from the code under test. If a
/// future edit breaks the bound, this fails rather than letting a `None`
/// reach a caller.
#[test]
fn blend_channel_is_total_and_exact_over_every_input() {
    assert_eq!(
        check_blend_channel_is_total_and_exact_over_every_input(),
        Ok(())
    );
}

fn check_blend_channel_is_total_and_exact_over_every_input() -> Result<(), Fail> {
    for alpha in 0..=u8::MAX {
        let inv = u8::MAX.checked_sub(alpha).or_fail()?;
        for src in 0..=u8::MAX {
            for dst in 0..=u8::MAX {
                let got = blend_rgb(
                    Rgb8 {
                        red: Channel(src),
                        green: Channel(src),
                        blue: Channel(src),
                    },
                    Alpha(alpha),
                    Rgb8 {
                        red: Channel(dst),
                        green: Channel(dst),
                        blue: Channel(dst),
                    },
                )
                .or_fail()?;
                let from_src = u64::from(src).checked_mul(u64::from(alpha)).or_fail()?;
                let from_dst = u64::from(dst).checked_mul(u64::from(inv)).or_fail()?;
                let total = from_src
                    .checked_add(from_dst)
                    .and_then(|sum| sum.checked_add(127))
                    .or_fail()?;
                let expected = u8::try_from(total.checked_div(255).or_fail()?).or_fail()?;
                ensure_eq!(
                    got,
                    Rgb8 {
                        red: Channel(expected),
                        green: Channel(expected),
                        blue: Channel(expected)
                    },
                    "blend(src={src}, dst={dst}, alpha={alpha})"
                );
            }
        }
    }
    Ok(())
}
