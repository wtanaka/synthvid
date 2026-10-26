use super::*;
use crate::deep::TrigNumerator;
use core::num::{NonZeroU32, NonZeroU8};
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// A lens sample from its across and down numerators on a grid.
fn sample(numerators: LensNumerators, grid: ApertureGrid) -> Result<LensSample, Fail> {
    LensSample::new(numerators, grid).or_fail()
}

/// The aperture points of an outline on a grid.
fn points(shape: ApertureShape, grid: ApertureGrid) -> Result<AperturePoints, Fail> {
    aperture_points(shape, grid).or_fail()
}

/// A polygon outline with the given number of blades.
fn polygon(blades: NonZeroU8) -> Result<ApertureShape, Fail> {
    Ok(ApertureShape::Polygon(BladeCount::new(blades).or_fail()?))
}

#[test]
fn disc_with_two_samples_keeps_the_four_corners_in_row_order() {
    assert_eq!(
        check_disc_with_two_samples_keeps_the_four_corners_in_row_order(),
        Ok(())
    );
}

fn check_disc_with_two_samples_keeps_the_four_corners_in_row_order() -> Result<(), Fail> {
    let found = points(ApertureShape::Disc, ApertureGrid::Two)?;
    let grid = ApertureGrid::Two;
    let expected = [
        sample(
            LensNumerators {
                across: LensNumerator(-1),
                down: LensNumerator(-1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(1),
                down: LensNumerator(-1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(-1),
                down: LensNumerator(1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(1),
                down: LensNumerator(1),
            },
            grid,
        )?,
    ];
    ensure_eq!(found.samples(), expected.as_slice());
    ensure_eq!(found.grid(), ApertureGrid::Two);
    Ok(())
}

#[test]
fn disc_with_four_samples_drops_the_corners() {
    assert_eq!(check_disc_with_four_samples_drops_the_corners(), Ok(()));
}

fn check_disc_with_four_samples_drops_the_corners() -> Result<(), Fail> {
    let grid = ApertureGrid::Four;
    let found = points(ApertureShape::Disc, grid)?;
    ensure_eq!(found.samples().len(), 12);
    for (across, down) in [(3, 3), (3, -3), (-3, 3), (-3, -3)] {
        let corner = LensNumerators {
            across: LensNumerator(across),
            down: LensNumerator(down),
        };
        ensure!(!found.samples().contains(&sample(corner, grid)?));
    }
    for (across, down) in [(1, 3), (3, 1), (-1, -3), (-3, -1)] {
        let edge = LensNumerators {
            across: LensNumerator(across),
            down: LensNumerator(down),
        };
        ensure!(found.samples().contains(&sample(edge, grid)?));
    }
    Ok(())
}

#[test]
fn disc_with_eight_samples_keeps_fifty_two_points() {
    assert_eq!(
        check_disc_with_eight_samples_keeps_fifty_two_points(),
        Ok(())
    );
}

fn check_disc_with_eight_samples_keeps_fifty_two_points() -> Result<(), Fail> {
    let grid = ApertureGrid::Eight;
    let found = points(ApertureShape::Disc, grid)?;
    ensure_eq!(found.samples().len(), 52);
    let kept = LensNumerators {
        across: LensNumerator(7),
        down: LensNumerator(1),
    };
    ensure!(found.samples().contains(&sample(kept, grid)?));
    let dropped = LensNumerators {
        across: LensNumerator(7),
        down: LensNumerator(5),
    };
    ensure!(!found.samples().contains(&sample(dropped, grid)?));
    Ok(())
}

#[test]
fn four_blade_diamond_with_four_samples_keeps_twelve_points() {
    assert_eq!(
        check_four_blade_diamond_with_four_samples_keeps_twelve_points(),
        Ok(())
    );
}

fn check_four_blade_diamond_with_four_samples_keeps_twelve_points() -> Result<(), Fail> {
    let grid = ApertureGrid::Four;
    let found = points(polygon(NonZeroU8::new(4).or_fail()?)?, grid)?;
    ensure_eq!(found.samples().len(), 12);
    let corner = LensNumerators {
        across: LensNumerator(3),
        down: LensNumerator(3),
    };
    ensure!(!found.samples().contains(&sample(corner, grid)?));
    let on_edge = LensNumerators {
        across: LensNumerator(1),
        down: LensNumerator(3),
    };
    ensure!(found.samples().contains(&sample(on_edge, grid)?));
    Ok(())
}

#[test]
fn four_blade_diamond_with_two_samples_keeps_the_points_on_its_edges() {
    assert_eq!(
        check_four_blade_diamond_with_two_samples_keeps_the_points_on_its_edges(),
        Ok(())
    );
}

fn check_four_blade_diamond_with_two_samples_keeps_the_points_on_its_edges() -> Result<(), Fail> {
    let grid = ApertureGrid::Two;
    let found = points(polygon(NonZeroU8::new(4).or_fail()?)?, grid)?;
    let expected = [
        sample(
            LensNumerators {
                across: LensNumerator(-1),
                down: LensNumerator(-1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(1),
                down: LensNumerator(-1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(-1),
                down: LensNumerator(1),
            },
            grid,
        )?,
        sample(
            LensNumerators {
                across: LensNumerator(1),
                down: LensNumerator(1),
            },
            grid,
        )?,
    ];
    ensure_eq!(found.samples(), expected.as_slice());
    Ok(())
}

#[test]
fn six_blade_polygon_with_eight_samples_keeps_forty_points() {
    assert_eq!(
        check_six_blade_polygon_with_eight_samples_keeps_forty_points(),
        Ok(())
    );
}

fn check_six_blade_polygon_with_eight_samples_keeps_forty_points() -> Result<(), Fail> {
    // Inside a regular hexagon: |b| <= 5, and sqrt(3) * |a| + |b| <= 8 * sqrt(3). That admits
    // 8 values of a when |b| = 1, and 6 values when |b| = 3 or |b| = 5, on each sign of b.
    let grid = ApertureGrid::Eight;
    let hexagon = points(polygon(NonZeroU8::new(6).or_fail()?)?, grid)?;
    let disc = points(ApertureShape::Disc, grid)?;
    ensure_eq!(hexagon.samples().len(), 40);
    ensure!(hexagon.samples().len() < disc.samples().len());
    Ok(())
}

#[test]
fn disc_samples_are_symmetric_under_each_reflection() {
    assert_eq!(
        check_disc_samples_are_symmetric_under_each_reflection(),
        Ok(())
    );
}

fn check_disc_samples_are_symmetric_under_each_reflection() -> Result<(), Fail> {
    let grid = ApertureGrid::Eight;
    let found = points(ApertureShape::Disc, grid)?;
    for kept in found.samples() {
        let across = kept.across();
        let down = kept.down();
        let flipped_across = LensNumerator(across.0.checked_neg().or_fail()?);
        let flipped_down = LensNumerator(down.0.checked_neg().or_fail()?);
        let mirrored = [
            LensNumerators {
                across: flipped_across,
                down,
            },
            LensNumerators {
                across,
                down: flipped_down,
            },
            LensNumerators {
                across: flipped_across,
                down: flipped_down,
            },
        ];
        for numerators in mirrored {
            ensure!(found.samples().contains(&sample(numerators, grid)?));
        }
    }
    Ok(())
}

#[test]
fn quarter_turn_converts_to_exact_cosine_and_sine_numerators() {
    assert_eq!(
        check_quarter_turn_converts_to_exact_cosine_and_sine_numerators(),
        Ok(())
    );
}

fn check_quarter_turn_converts_to_exact_cosine_and_sine_numerators() -> Result<(), Fail> {
    let quarter = NonZeroI64::new(4).or_fail()?;
    // The roll is the negated angle, so this pair is the cosine and sine of a quarter turn.
    let roll = Turns::new(Ratio::new(Numerator(-1), quarter).or_fail()?);
    let pair = TrigPair::of(roll).or_fail()?;
    ensure_eq!(pair.cosine(), TrigNumerator(0));
    ensure_eq!(pair.sine(), TrigNumerator(0x8000));
    Ok(())
}

#[test]
fn point_counts_for_every_grid_and_outline() {
    assert_eq!(check_point_counts_for_every_grid_and_outline(), Ok(()));
}

fn check_point_counts_for_every_grid_and_outline() -> Result<(), Fail> {
    for grid in [ApertureGrid::Two, ApertureGrid::Four, ApertureGrid::Eight] {
        // The disc keeps the lattice points inside the unit circle: 4 on the two by two grid, 12 on
        // the four by four grid, and 52 on the eight by eight grid. The disc is symmetric under
        // reflection through the center, so every inside point keeps its reflection too.
        let expected_disc: u32 = match grid {
            ApertureGrid::Two => 4,
            ApertureGrid::Four => 12,
            ApertureGrid::Eight => 52,
        };
        ensure_eq!(
            points(ApertureShape::Disc, grid)?.count(),
            LensPointCount::new(NonZeroU32::new(expected_disc).or_fail()?)
        );
    }
    Ok(())
}

#[test]
fn every_outline_on_every_grid_is_centered_or_refused() {
    assert_eq!(
        check_every_outline_on_every_grid_is_centered_or_refused(),
        Ok(())
    );
}

fn check_every_outline_on_every_grid_is_centered_or_refused() -> Result<(), Fail> {
    // Each kept candidate comes with its reflection, so the numerators of the kept set sum to zero
    // on each axis. The only other outcome is the refusal of an outline with no symmetric pair.
    for grid in [ApertureGrid::Two, ApertureGrid::Four, ApertureGrid::Eight] {
        let mut shapes = vec![ApertureShape::Disc];
        for blades in 3_u8..=9 {
            shapes.push(polygon(NonZeroU8::new(blades).or_fail()?)?);
        }
        for shape in shapes {
            match aperture_points(shape, grid) {
                Ok(found) => {
                    let mut across_sum = 0_i64;
                    let mut down_sum = 0_i64;
                    for kept in found.samples() {
                        across_sum = across_sum.checked_add(kept.across().0).or_fail()?;
                        down_sum = down_sum.checked_add(kept.down().0).or_fail()?;
                    }
                    ensure_eq!(across_sum, 0);
                    ensure_eq!(down_sum, 0);
                }
                Err(error) => ensure_eq!(error, ApertureError::NoSymmetricPoints),
            }
        }
    }
    Ok(())
}

#[test]
fn three_blades_on_the_two_by_two_grid_are_refused() {
    assert_eq!(
        check_three_blades_on_the_two_by_two_grid_are_refused(),
        Ok(())
    );
}

fn check_three_blades_on_the_two_by_two_grid_are_refused() -> Result<(), Fail> {
    // The triangle keeps (-1/2, -1/2) and (-1/2, 1/2), but their reflections (1/2, 1/2) and
    // (1/2, -1/2) lie outside it, so no candidate has an inside reflection and the set is empty.
    let triangle = polygon(NonZeroU8::new(3).or_fail()?)?;
    ensure_eq!(
        aperture_points(triangle, ApertureGrid::Two),
        Err(ApertureError::NoSymmetricPoints)
    );
    Ok(())
}

#[test]
fn five_blades_on_the_four_by_four_grid_drop_only_the_corners() {
    assert_eq!(
        check_five_blades_on_the_four_by_four_grid_drop_only_the_corners(),
        Ok(())
    );
}

fn check_five_blades_on_the_four_by_four_grid_drop_only_the_corners() -> Result<(), Fail> {
    // Candidates are (a/4, b/4) with a and b in {-3, -1, 1, 3}. A point is inside the pentagon,
    // whose apothem is cos 36 deg, about 0.80902, when its projection on each edge normal is at most
    // the apothem. The corner (3/4, 3/4) projects to 0.60676 + 0.44084 = 1.0476 on the normal at 36
    // degrees, so it is outside, and so are its three sign images. Every other candidate projects
    // to at most 0.7906 on every normal, and the outline is symmetric under each axis reflection, so
    // the twelve other candidates are kept.
    let grid = ApertureGrid::Four;
    let found = points(polygon(NonZeroU8::new(5).or_fail()?)?, grid)?;
    ensure_eq!(
        found.count(),
        LensPointCount::new(NonZeroU32::new(12).or_fail()?)
    );
    for (across, down) in [(3, 3), (3, -3), (-3, 3), (-3, -3)] {
        let corner = LensNumerators {
            across: LensNumerator(across),
            down: LensNumerator(down),
        };
        ensure!(!found.samples().contains(&sample(corner, grid)?));
    }
    Ok(())
}

/// The numerators of the candidate at the upper left of the two by two grid, -1 across and 1 down.
fn upper_left_numerators() -> LensNumerators {
    LensNumerators {
        across: LensNumerator(-1),
        down: LensNumerator(1),
    }
}

/// The numerators of the candidate at the lower right of the two by two grid, 1 across and -1
/// down. It is the point reflection of [`upper_left_numerators`].
fn lower_right_numerators() -> LensNumerators {
    LensNumerators {
        across: LensNumerator(1),
        down: LensNumerator(-1),
    }
}

#[test]
fn four_blade_diamond_covers_the_upper_left_candidate_on_its_edge() {
    assert_eq!(
        check_four_blade_diamond_covers_the_upper_left_candidate_on_its_edge(),
        Ok(())
    );
}

fn check_four_blade_diamond_covers_the_upper_left_candidate_on_its_edge() -> Result<(), Fail> {
    // The diamond has vertices (1, 0), (0, 1), (-1, 0) and (0, -1) in unit coordinates, so its
    // edges lie on |x| + |y| = 1. The upper left candidate is (-1/2, 1/2), and |-1/2| + |1/2| = 1,
    // so it lies exactly on the edge. The vertices at quarter turns are exact, with cosine 0 and sine
    // of magnitude 1, so the lattice edge is |x| + |y| = 2^15 and the candidate sits at
    // (-16384, 16384) on it. It is inside because edges count as inside, and so is its reflection,
    // which lies on the opposite edge, so the diamond keeps both.
    let shape = polygon(NonZeroU8::new(4).or_fail()?)?;
    ensure_eq!(
        candidate_inside(shape, ApertureGrid::Two, upper_left_numerators()).or_fail()?,
        Containment::Inside
    );
    ensure_eq!(
        candidate_inside(shape, ApertureGrid::Two, lower_right_numerators()).or_fail()?,
        Containment::Inside
    );
    Ok(())
}

#[test]
fn triangle_candidate_is_inside_by_one_lattice_unit_but_its_reflection_is_outside() {
    assert_eq!(
        check_triangle_candidate_is_inside_by_one_lattice_unit_but_its_reflection_is_outside(),
        Ok(())
    );
}

fn check_triangle_candidate_is_inside_by_one_lattice_unit_but_its_reflection_is_outside(
) -> Result<(), Fail> {
    // Vertex k of the triangle lies at k/3 turns. The vertex at one third of a turn has cosine
    // -1/2, which is -16384 on the 2^15 scale, but the trigonometry of the scene rounds it to
    // -16385. The left edge of the triangle is therefore the vertical line across = -16385, and
    // the upper left candidate at across = -16384 lies one lattice unit inside it. Its reflection,
    // (16384, -16384), lies beyond the edge from the vertex at zero turns to the vertex at two
    // thirds of a turn: that edge passes through (16384, -9459), and -16384 is farther down, so the
    // reflection is outside and the candidate is not kept.
    let shape = polygon(NonZeroU8::new(3).or_fail()?)?;
    let one_third = TrigPair::of(Turns::new(
        Ratio::new(Numerator(-1), NonZeroI64::new(3).or_fail()?).or_fail()?,
    ))
    .or_fail()?;
    ensure_eq!(one_third.cosine(), TrigNumerator(-0x4001));
    ensure_eq!(
        candidate_inside(shape, ApertureGrid::Two, upper_left_numerators()).or_fail()?,
        Containment::Inside
    );
    ensure_eq!(
        candidate_inside(shape, ApertureGrid::Two, lower_right_numerators()).or_fail()?,
        Containment::Outside
    );
    Ok(())
}

#[test]
fn five_blades_on_the_eight_by_eight_grid_keep_thirty_two_points() {
    assert_eq!(
        check_five_blades_on_the_eight_by_eight_grid_keep_thirty_two_points(),
        Ok(())
    );
}

fn check_five_blades_on_the_eight_by_eight_grid_keep_thirty_two_points() -> Result<(), Fail> {
    // Hand derivation, in unit coordinates p = (x, y) = (a/8, b/8). The pentagon has apothem
    // cos 36 deg, about 0.80902, and its edge normals lie at 36, 108, 180, 252 and 324 degrees. A
    // candidate and its reflection both lie inside exactly when |p . n| <= 0.80902 for every normal
    // n, which gives these five tests:
    //   |x| <= 0.80902                       (normal at 180 degrees)
    //   |0.80902 x + 0.58779 y| <= 0.80902   (normal at 36 degrees)
    //   |0.80902 x - 0.58779 y| <= 0.80902   (normal at 324 degrees)
    //   |0.30902 x - 0.95106 y| <= 0.80902   (normal at 108 degrees)
    //   |0.30902 x + 0.95106 y| <= 0.80902   (normal at 252 degrees)
    // The five tests are unchanged by flipping the sign of x or of y, because the normals form the
    // same set of lines under each axis reflection, so it is enough to decide a, b > 0. There a = 7
    // fails the first test, since 7/8 > 0.80902. With a = 1 and b = 7 the test at 252 degrees gives
    // 0.87082, and with a = 3 and b = 7 the test at 36 degrees gives 0.81769, both above the apothem.
    // With b = 5 the test at 36 degrees gives 0.87300 for a = 5, which fails, and 0.67074 for a = 3,
    // which passes. The survivors with a, b > 0 are therefore
    //   a = 1: b = 1, 3, 5
    //   a = 3: b = 1, 3, 5
    //   a = 5: b = 1, 3
    // which is 8 points, and each sign pattern adds 8 more, for 32 in all.
    let grid = ApertureGrid::Eight;
    let found = points(polygon(NonZeroU8::new(5).or_fail()?)?, grid)?;
    ensure_eq!(
        found.count(),
        LensPointCount::new(NonZeroU32::new(32).or_fail()?)
    );
    let survivors: [(i64, i64); 8] = [
        (1, 1),
        (1, 3),
        (1, 5),
        (3, 1),
        (3, 3),
        (3, 5),
        (5, 1),
        (5, 3),
    ];
    for (across, down) in survivors {
        let flipped_across = across.checked_neg().or_fail()?;
        let flipped_down = down.checked_neg().or_fail()?;
        for (signed_across, signed_down) in [
            (across, down),
            (flipped_across, down),
            (across, flipped_down),
            (flipped_across, flipped_down),
        ] {
            let numerators = LensNumerators {
                across: LensNumerator(signed_across),
                down: LensNumerator(signed_down),
            };
            ensure!(found.samples().contains(&sample(numerators, grid)?));
        }
    }
    let excluded: [(i64, i64); 4] = [(5, 5), (1, 7), (3, 7), (7, 1)];
    for (across, down) in excluded {
        let numerators = LensNumerators {
            across: LensNumerator(across),
            down: LensNumerator(down),
        };
        ensure!(!found.samples().contains(&sample(numerators, grid)?));
    }
    Ok(())
}

#[test]
fn repeated_calls_give_identical_points() {
    assert_eq!(check_repeated_calls_give_identical_points(), Ok(()));
}

fn check_repeated_calls_give_identical_points() -> Result<(), Fail> {
    let grid = ApertureGrid::Eight;
    let shape = polygon(NonZeroU8::new(6).or_fail()?)?;
    let first = points(shape, grid)?;
    let second = points(shape, grid)?;
    ensure_eq!(first, second);
    Ok(())
}
