use super::super::{average_channel, ChannelSum, SampleCount};
use core::num::NonZeroU32;
use synthvid_scene::Channel;
use synthvid_testkit::{ensure_eq, Fail, OrFail as _};

#[test]
fn channel_average_rounds_half_up_over_the_samples() {
    assert_eq!(check_channel_average_rounds_half_up(), Ok(()));
}

fn check_channel_average_rounds_half_up() -> Result<(), Fail> {
    // Five samples of 200 and seven of 10: 1070 over 12 is 89.17, which rounds to 89.
    let twelve = SampleCount(NonZeroU32::new(12).or_fail()?);
    ensure_eq!(average_channel(ChannelSum(1070), twelve), Ok(Channel(89)));
    // 255 over 4 is 63.75, which rounds to 64.
    let four = SampleCount(NonZeroU32::new(4).or_fail()?);
    ensure_eq!(average_channel(ChannelSum(255), four), Ok(Channel(64)));
    ensure_eq!(average_channel(ChannelSum(0), four), Ok(Channel(0)));
    // Full scale stays at full scale for every count, since 255 is the largest sample.
    // Twelve samples of 255 sum to 3060, and sixty-four samples of 255 sum to 16320.
    ensure_eq!(average_channel(ChannelSum(3060), twelve), Ok(Channel(255)));
    let sixty_four = SampleCount(NonZeroU32::new(64).or_fail()?);
    ensure_eq!(
        average_channel(ChannelSum(16320), sixty_four),
        Ok(Channel(255))
    );
    // 3 over 2 is 1.5, which rounds to 2; 1 over 2 is 0.5, which rounds to 1.
    let two = SampleCount(NonZeroU32::new(2).or_fail()?);
    ensure_eq!(average_channel(ChannelSum(3), two), Ok(Channel(2)));
    ensure_eq!(average_channel(ChannelSum(1), two), Ok(Channel(1)));
    // 1 over 3 is 0.33, which rounds to 0.
    let three = SampleCount(NonZeroU32::new(3).or_fail()?);
    ensure_eq!(average_channel(ChannelSum(1), three), Ok(Channel(0)));
    Ok(())
}
