//! Tests that render catalogue entries through a video specification and check the bytes.

use crate::cover::{
    avi_truncated_entry, entry_like, generate_catalogue, AxisValues, BackgroundAxis, CodingAxis,
    ContainerAxis, EntrySize, FrameRateValue, MotionAxis, ScaleAxis, ShapeAxis, TrackMatrixAxis,
};
use crate::generator::generate_entry;
use crate::sha256;
use synthvid_encode::defects::{BoxPath, BoxTag, Defect};
use synthvid_testkit::{ensure_eq, ensure_ne, Fail, OrFail as _};
use synthvid_video::{render, Container, DisplayRotation, Encoding};

#[test]
fn frozen_digest_mjpeg_avi_rot90() {
    assert_eq!(check_frozen_digest_mjpeg_avi_rot90(), Ok(()));
}

fn check_frozen_digest_mjpeg_avi_rot90() -> Result<(), Fail> {
    let entry = avi_truncated_entry()?;

    let generated = generate_entry(&entry).or_fail()?;
    let manifest_digest = sha256(generated.manifest_json().as_str().as_bytes()).to_string();
    let media_digest = sha256(generated.media_bytes().as_bytes()).to_string();

    ensure_eq!(
        manifest_digest,
        "bd9419358c2218049e88ca8ea0987ced463534ea1d0db9c36314bef5daae3c18"
    );
    ensure_eq!(
        media_digest,
        "a7a9ef428a03c7f34b0be2d622afe37cf1898d871de3970d44d1af2f9f422167"
    );
    ensure_eq!(generated.manifest_json().as_str().len(), 4231);
    // The RIFF header (12 bytes) and the `hdrl` list (200 bytes) come
    // before the `movi` list, which is where the cut falls.
    ensure_eq!(generated.media_bytes().as_bytes().len(), 212);
    // The cut file is a prefix of the same file without the defect.
    let mut whole_entry = entry;
    whole_entry.set_defect(None);
    let whole = generate_entry(&whole_entry).or_fail()?;
    ensure_eq!(
        generated.media_bytes().as_bytes(),
        whole.media_bytes().as_bytes().get(..212).or_fail()?
    );
    Ok(())
}

#[test]
fn frozen_digest_raw_iso_rot270() {
    assert_eq!(check_frozen_digest_raw_iso_rot270(), Ok(()));
}

fn check_frozen_digest_raw_iso_rot270() -> Result<(), Fail> {
    let entry = entry_like(EntrySize::Tiny10, |entry| {
        entry.set_axes(AxisValues {
            frame_rate: FrameRateValue::Ntsc,
            background: BackgroundAxis::Gradient,
            shape: ShapeAxis::Polygon,
            motion: MotionAxis::Fixed,
            coding: CodingAxis::Raw,
            container: ContainerAxis::Iso,
            track_matrix: TrackMatrixAxis::Rotate270,
            scale: ScaleAxis::Undeclared,
            defect: Some(Defect::ReorderedBoxes(
                BoxPath::root().child(BoxTag::new(*b"moov")),
                BoxPath::root().child(BoxTag::new(*b"mdat")),
            )),
        });
    })?;

    let generated = generate_entry(&entry).or_fail()?;
    let manifest_digest = sha256(generated.manifest_json().as_str().as_bytes()).to_string();
    let media_digest = sha256(generated.media_bytes().as_bytes()).to_string();

    ensure_eq!(
        manifest_digest,
        "45e6a9fe7be0bb352ac7f7ec3460dca5dcf3576af973817535805630fa3587c1"
    );
    ensure_eq!(
        media_digest,
        "a3b0fc24629da5fad7d776ce58d7d3e1619072385a84d0f67ca7ce5c70472fa7"
    );
    ensure_eq!(generated.manifest_json().as_str().len(), 4253);
    ensure_eq!(generated.media_bytes().as_bytes().len(), 8350);
    Ok(())
}

#[test]
fn frozen_digest_mjpeg_iso_rot180() {
    assert_eq!(check_frozen_digest_mjpeg_iso_rot180(), Ok(()));
}

fn check_frozen_digest_mjpeg_iso_rot180() -> Result<(), Fail> {
    let entry = entry_like(EntrySize::Qvga10, |entry| {
        entry.set_axes(AxisValues {
            frame_rate: FrameRateValue::Thirty,
            background: BackgroundAxis::Grid,
            shape: ShapeAxis::Cross,
            motion: MotionAxis::Linear,
            coding: CodingAxis::MotionJpeg,
            container: ContainerAxis::Iso,
            track_matrix: TrackMatrixAxis::Rotate180,
            scale: ScaleAxis::Declared,
            defect: Some(Defect::DuplicateBox(
                BoxPath::root()
                    .child(BoxTag::new(*b"moov"))
                    .child(BoxTag::new(*b"trak")),
            )),
        });
    })?;

    let generated = generate_entry(&entry).or_fail()?;
    let manifest_digest = sha256(generated.manifest_json().as_str().as_bytes()).to_string();
    let media_digest = sha256(generated.media_bytes().as_bytes()).to_string();

    ensure_eq!(
        manifest_digest,
        "6bcaedba7ac92261afd841376c1feb984f6ae1d76288515ec5362bdac94e1840"
    );
    ensure_eq!(
        media_digest,
        "f0200ae32c2a7287b8764d81f9b22ebc48f585c3d0cc1fc0c0bbb0df1fedc0f4"
    );
    ensure_eq!(generated.manifest_json().as_str().len(), 4_913);
    ensure_eq!(generated.media_bytes().as_bytes().len(), 95_005);
    Ok(())
}

#[test]
fn frozen_digest_raw_iso_rot180_zero_fps_defect() {
    assert_eq!(check_frozen_digest_raw_iso_rot180_zero_fps_defect(), Ok(()));
}

fn check_frozen_digest_raw_iso_rot180_zero_fps_defect() -> Result<(), Fail> {
    let entry = entry_like(EntrySize::Tiny1, |entry| {
        entry.set_axes(AxisValues {
            frame_rate: FrameRateValue::Thirty,
            background: BackgroundAxis::Gradient,
            shape: ShapeAxis::Disc,
            motion: MotionAxis::Fixed,
            coding: CodingAxis::Raw,
            container: ContainerAxis::Iso,
            track_matrix: TrackMatrixAxis::Rotate180,
            scale: ScaleAxis::Declared,
            defect: Some(Defect::ZeroFrameRate),
        });
    })?;

    let generated = generate_entry(&entry).or_fail()?;
    let manifest_digest = sha256(generated.manifest_json().as_str().as_bytes()).to_string();
    let media_digest = sha256(generated.media_bytes().as_bytes()).to_string();

    ensure_eq!(
        manifest_digest,
        "e52713819ed859efd82877cb39b047d0e07547f1e3b6d7e95f6448d9d9abff6c"
    );
    ensure_eq!(
        media_digest,
        "c206d384d6b32232ebcb4d40944f3b9f9524bef0b3786391fb5843a51a5f3eab"
    );
    ensure_eq!(generated.manifest_json().as_str().len(), 770);
    ensure_eq!(generated.media_bytes().as_bytes().len(), 1366);
    Ok(())
}

#[test]
fn name_and_axis_mapping_for_16x16() {
    assert_eq!(check_16x16_name_and_axis_mapping(), Ok(()));
}

fn check_16x16_name_and_axis_mapping() -> Result<(), Fail> {
    let entries = generate_catalogue().or_fail()?;

    for entry in &entries {
        if entry.manifest_name().as_str().starts_with("0016x0016") {
            let spec = entry.to_video_spec().or_fail()?;
            ensure_eq!(spec.name().as_str(), entry.manifest_name().as_str());

            match entry.coding() {
                CodingAxis::Raw => {
                    ensure_eq!(spec.encoding(), Encoding::Raw);
                }
                CodingAxis::MotionJpeg => {
                    ensure_ne!(spec.encoding(), Encoding::Raw);
                }
            }

            let rotation = match entry.track_matrix() {
                TrackMatrixAxis::Identity => DisplayRotation::Identity,
                TrackMatrixAxis::Rotate90 => DisplayRotation::Rot90,
                TrackMatrixAxis::Rotate180 => DisplayRotation::Rot180,
                TrackMatrixAxis::Rotate270 => DisplayRotation::Rot270,
            };
            match entry.container() {
                ContainerAxis::Iso => {
                    ensure_eq!(spec.container(), Container::Iso { rotation });
                }
                ContainerAxis::Avi => {
                    ensure_eq!(spec.container(), Container::Avi);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn rotation_changes_iso_but_not_manifest() {
    assert_eq!(check_rotation_changes_iso_but_not_manifest(), Ok(()));
}

fn check_rotation_changes_iso_but_not_manifest() -> Result<(), Fail> {
    let entries = generate_catalogue().or_fail()?;
    let mut rotated_spec = None;
    for entry in &entries {
        let spec = entry.to_video_spec().or_fail()?;
        let rotated_iso = [
            DisplayRotation::Rot90,
            DisplayRotation::Rot180,
            DisplayRotation::Rot270,
        ]
        .into_iter()
        .any(|rotation| spec.container() == Container::Iso { rotation });
        if rotated_iso && spec.defect().is_none() {
            rotated_spec = Some(spec);
            break;
        }
    }
    let spec = rotated_spec.or_fail()?;

    let has_defect = spec.defect().is_some();
    let rotated = render(&spec).or_fail()?;
    let identity = render(&spec.with_container(Container::Iso {
        rotation: DisplayRotation::Identity,
    }))
    .or_fail()?;

    ensure_eq!(rotated.manifest_json(), identity.manifest_json());
    if !has_defect {
        ensure_ne!(rotated.media_bytes(), identity.media_bytes());
    }
    Ok(())
}

#[test]
fn avi_entry_carries_no_rotation() {
    assert_eq!(check_avi_entry_carries_no_rotation(), Ok(()));
}

fn check_avi_entry_carries_no_rotation() -> Result<(), Fail> {
    let entry = avi_truncated_entry()?;

    let spec = entry.to_video_spec().or_fail()?;
    ensure_eq!(spec.container(), Container::Avi);
    Ok(())
}

#[test]
fn frozen_digest_via_render_api() {
    assert_eq!(check_frozen_digest_via_render_api(), Ok(()));
}

fn check_frozen_digest_via_render_api() -> Result<(), Fail> {
    let entry = avi_truncated_entry()?;

    let spec = entry.to_video_spec().or_fail()?;
    let generated = render(&spec).or_fail()?;

    let manifest_digest = sha256(generated.manifest_json().as_str().as_bytes()).to_string();
    let media_digest = sha256(generated.media_bytes().as_bytes()).to_string();

    ensure_eq!(
        manifest_digest,
        "bd9419358c2218049e88ca8ea0987ced463534ea1d0db9c36314bef5daae3c18"
    );
    ensure_eq!(
        media_digest,
        "a7a9ef428a03c7f34b0be2d622afe37cf1898d871de3970d44d1af2f9f422167"
    );
    Ok(())
}
