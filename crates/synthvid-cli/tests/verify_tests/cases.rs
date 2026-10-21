//! Cases for the verify command, run against the built binary.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use synthvid_testkit::{ensure, Fail, OrFail as _};

/// Helper to get the synthvid-cli binary path.
fn synthvid_bin() -> PathBuf {
    let bin_name = env!("CARGO_BIN_EXE_synthvid-cli");
    PathBuf::from(bin_name)
}

/// Helper to get the catalog.lock path in the project root.
fn catalog_lock_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("../../catalog.lock")
}

#[test]
fn verify_only_tiny_entry_succeeds() {
    assert_eq!(check_verify_only_tiny_entry_succeeds(), Ok(()));
}

fn check_verify_only_tiny_entry_succeeds() -> Result<(), Fail> {
    // Test: verify --only <tiny-entry> against the real catalog.lock exits 0
    let lock_path = catalog_lock_path();
    ensure!(
        lock_path.exists(),
        "catalog.lock should exist at {lock_path:?}"
    );

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef")
        .arg("--lock")
        .arg(&lock_path)
        .output()
        .or_fail()?;

    ensure!(
        output.status.success(),
        "verify --only should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("ok 0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef"),
        "stdout should contain 'ok <entry>' for matching entry; got: {stdout}"
    );
    Ok(())
}

#[test]
fn verify_unknown_only_name_is_error() {
    assert_eq!(check_verify_unknown_only_name_is_error(), Ok(()));
}

fn check_verify_unknown_only_name_is_error() -> Result<(), Fail> {
    // Test: --only with an unknown name is an error
    let lock_path = catalog_lock_path();

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("this-entry-does-not-exist")
        .arg("--lock")
        .arg(&lock_path)
        .output()
        .or_fail()?;

    ensure!(
        !output.status.success(),
        "verify --only with unknown name should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("unknown entry name"),
        "stderr should contain 'unknown entry name'; got: {stderr}"
    );
    Ok(())
}

#[test]
fn verify_corpus_with_only_is_usage_error() {
    assert_eq!(check_verify_corpus_with_only_is_usage_error(), Ok(()));
}

fn check_verify_corpus_with_only_is_usage_error() -> Result<(), Fail> {
    // Test: --corpus combined with --only is a usage error
    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg("/tmp/nonexistent")
        .arg("--only")
        .arg("0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef")
        .output()
        .or_fail()?;

    ensure!(
        !output.status.success(),
        "verify --corpus with --only should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("--corpus cannot be combined with --only"),
        "stderr should contain usage error; got: {stderr}"
    );
    Ok(())
}

#[test]
fn verify_digest_mismatch_detection() {
    assert_eq!(check_verify_digest_mismatch_detection(), Ok(()));
}

fn check_verify_digest_mismatch_detection() -> Result<(), Fail> {
    // Test: a modified lockfile entry's manifest digest is detected as a mismatch
    let lock_path = catalog_lock_path();
    let lock_content = fs::read_to_string(&lock_path).or_fail()?;

    // Create a temp file with a modified digest for the test entry
    let temp_lock =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-mismatch.lock");
    let mut modified_content = lock_content
        .lines()
        .map(|line| {
            if line.starts_with(
                "0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef.json ",
            ) {
                // Replace the digest with a fake one
                let parts: Vec<&str> = line.split(' ').collect();
                if let [name, _, length] = parts.as_slice() {
                    return format!(
                        "{name} {} {length}",
                        "0000000000000000000000000000000000000000000000000000000000000000"
                    );
                }
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    modified_content.push('\n');

    fs::write(&temp_lock, modified_content).or_fail()?;

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef")
        .arg("--lock")
        .arg(&temp_lock)
        .output()
        .or_fail()?;

    // Clean up
    let removal = fs::remove_file(&temp_lock);
    drop(removal);

    ensure!(
        !output.status.success(),
        "verify should fail when digest mismatches"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("digest mismatch in 0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef.json"),
        "stderr should contain digest mismatch error; got: {stderr}"
    );
    Ok(())
}

#[test]
fn verify_only_ignores_unselected_lockfile_entries() {
    assert_eq!(
        check_verify_only_ignores_unselected_lockfile_entries(),
        Ok(())
    );
}

fn check_verify_only_ignores_unselected_lockfile_entries() -> Result<(), Fail> {
    // A lockfile entry for an entry that was not selected, and that the catalogue does not
    // produce, must not be reported as missing when `--only` selects a different entry.
    let tiny = "0016x0016-1-30fps-solid-disc-fixed-raw-iso-identity-scaled-nodef";
    let lock_content = fs::read_to_string(catalog_lock_path()).or_fail()?;
    let bogus_digest = "1".repeat(64);
    let extended = format!(
        "{lock_content}bogus-entry.json {bogus_digest} 10\nbogus-entry-media.mp4 {bogus_digest} 20\n"
    );
    let temp_lock = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-scope.lock");
    fs::write(&temp_lock, extended).or_fail()?;

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg(tiny)
        .arg("--lock")
        .arg(&temp_lock)
        .output()
        .or_fail()?;

    let removal = fs::remove_file(&temp_lock);
    drop(removal);

    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "an unselected extra lockfile entry must not fail verify; stderr: {stderr}"
    );
    ensure!(
        !stderr.contains("bogus-entry"),
        "unselected entries must not be reported; stderr: {stderr}"
    );
    Ok(())
}

#[test]
fn verify_corpus_without_a_lockfile_names_the_problem() {
    assert_eq!(
        check_verify_corpus_without_a_lockfile_names_the_problem(),
        Ok(())
    );
}

fn check_verify_corpus_without_a_lockfile_names_the_problem() -> Result<(), Fail> {
    let corpus = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-no-corpus");
    let removal = fs::remove_dir_all(&corpus);
    drop(removal);

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .or_fail()?;

    ensure!(!output.status.success(), "a missing corpus must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("failed to read lockfile"),
        "stderr should name the unreadable lockfile; got: {stderr}"
    );
    Ok(())
}

#[test]
fn verify_corpus_reports_a_lockfile_entry_with_no_file() {
    assert_eq!(
        check_verify_corpus_reports_a_lockfile_entry_with_no_file(),
        Ok(())
    );
}

fn check_verify_corpus_reports_a_lockfile_entry_with_no_file() -> Result<(), Fail> {
    let corpus = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-empty-corpus");
    let removal = fs::remove_dir_all(&corpus);
    drop(removal);
    fs::create_dir_all(&corpus).or_fail()?;
    let digest = "0".repeat(64);
    fs::write(
        corpus.join("catalog.lock"),
        format!("only-entry.json {digest} 1\n"),
    )
    .or_fail()?;

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .or_fail()?;

    let cleanup = fs::remove_dir_all(&corpus);
    drop(cleanup);

    ensure!(!output.status.success(), "a missing file must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        stderr.contains("only-entry.json"),
        "stderr should name the missing file; got: {stderr}"
    );
    Ok(())
}
