use super::*;

#[test]
fn pixel_centre_exact_values() {
    // Test that pixel_centre produces exactly (2x + 1) / 2 for x, y coordinates.
    let pt_origin = pixel_centre(Column(0), Row(0));
    assert_eq!(pt_origin.across.numer().0, 1);
    assert_eq!(pt_origin.across.denom().get(), 2);
    assert_eq!(pt_origin.down.numer().0, 1);
    assert_eq!(pt_origin.down.denom().get(), 2);

    // Test boundary values: u16::MAX.
    let pt_max = pixel_centre(Column(u16::MAX), Row(u16::MAX));
    // For x = u16::MAX = 65535: (2 * 65535 + 1) / 2 = 0x1_ffff / 2.
    assert_eq!(pt_max.across.numer().0, 0x1_ffff);
    assert_eq!(pt_max.across.denom().get(), 2);
    assert_eq!(pt_max.down.numer().0, 0x1_ffff);
    assert_eq!(pt_max.down.denom().get(), 2);
}
