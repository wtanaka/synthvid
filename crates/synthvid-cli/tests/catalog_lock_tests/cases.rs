//! Cases that compare the catalogue's entry names with the committed `catalog.lock`.

extern crate alloc;

use alloc::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use synthvid_catalog::cover::generate_catalogue;
use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

/// The text of the `catalog.lock` in the project root.
struct Lockfile(String);

/// Reads the `catalog.lock` in the project root.
fn committed_lockfile() -> Result<Lockfile, Fail> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let text =
        fs::read_to_string(PathBuf::from(manifest_dir).join("../../catalog.lock")).or_fail()?;
    Ok(Lockfile(text))
}

fn check_playback_only_entries_are_in_committed_lockfile() -> Result<(), Fail> {
    let lockfile = committed_lockfile()?;
    // Each lockfile line is `name digest length`; the name is the first field.
    let lockfile_names: BTreeSet<&str> = lockfile
        .0
        .lines()
        .map(|line| match line.split_once(' ') {
            Some((name, _)) => name,
            None => line,
        })
        .collect();
    let catalogue = generate_catalogue().or_fail()?;
    let playback_only: Vec<_> = catalogue
        .iter()
        .filter(|entry| entry.capture().distinct().is_none())
        .collect();
    let captured = catalogue
        .iter()
        .filter(|entry| entry.capture().distinct().is_some())
        .count();
    ensure!(
        !playback_only.is_empty(),
        "the catalogue has playback-only entries"
    );
    ensure_eq!(
        playback_only.len(),
        catalogue.len().checked_sub(captured).or_fail()?
    );
    for entry in &playback_only {
        let manifest = format!("{}.json", entry.manifest_name());
        let media = entry.media_name().to_string();
        ensure!(
            lockfile_names.contains(manifest.as_str()),
            "entry without a capture rate must keep its manifest name: {manifest}"
        );
        ensure!(
            lockfile_names.contains(media.as_str()),
            "entry without a capture rate must keep its media name: {media}"
        );
    }
    Ok(())
}

#[test]
fn playback_only_entries_are_in_committed_lockfile() {
    assert_eq!(
        check_playback_only_entries_are_in_committed_lockfile(),
        Ok(())
    );
}
