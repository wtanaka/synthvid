//! Integration tests for the verify command.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
fn test_verify_only_tiny_entry_succeeds() {
    // Test: verify --only <tiny-entry> against the real catalog.lock exits 0
    let lock_path = catalog_lock_path();
    assert!(
        lock_path.exists(),
        "catalog.lock should exist at {lock_path:?}"
    );

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps")
        .arg("--lock")
        .arg(&lock_path)
        .output()
        .expect("failed to run synthvid-cli");

    assert!(
        output.status.success(),
        "verify --only should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ok 0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps"),
        "stdout should contain 'ok <entry>' for matching entry; got: {stdout}"
    );
}

#[test]
fn test_verify_unknown_only_name_is_error() {
    // Test: --only with an unknown name is an error
    let lock_path = catalog_lock_path();

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("this-entry-does-not-exist")
        .arg("--lock")
        .arg(&lock_path)
        .output()
        .expect("failed to run synthvid-cli");

    assert!(
        !output.status.success(),
        "verify --only with unknown name should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown entry name"),
        "stderr should contain 'unknown entry name'; got: {stderr}"
    );
}

#[test]
fn test_verify_corpus_with_only_is_usage_error() {
    // Test: --corpus combined with --only is a usage error
    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg("/tmp/nonexistent")
        .arg("--only")
        .arg("0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps")
        .output()
        .expect("failed to run synthvid-cli");

    assert!(
        !output.status.success(),
        "verify --corpus with --only should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--corpus cannot be combined with --only"),
        "stderr should contain usage error; got: {stderr}"
    );
}

#[test]
fn test_verify_digest_mismatch_detection() {
    // Test: a modified lockfile entry's manifest digest is detected as a mismatch
    let lock_path = catalog_lock_path();
    let lock_content = fs::read_to_string(&lock_path).expect("failed to read catalog.lock");

    // Create a temp file with a modified digest for the test entry
    let temp_lock =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-mismatch.lock");
    let modified_content = lock_content
        .lines()
        .map(|line| {
            if line.starts_with(
                "0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps.json ",
            ) {
                // Replace the digest with a fake one
                let parts: Vec<&str> = line.split(' ').collect();
                if parts.len() == 3 {
                    return format!(
                        "{} {} {}",
                        parts[0],
                        "0000000000000000000000000000000000000000000000000000000000000000",
                        parts[2]
                    );
                }
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    fs::write(&temp_lock, modified_content).expect("failed to write temp lockfile");

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg("0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps")
        .arg("--lock")
        .arg(&temp_lock)
        .output()
        .expect("failed to run synthvid-cli");

    // Clean up
    drop(fs::remove_file(&temp_lock));

    assert!(
        !output.status.success(),
        "verify should fail when digest mismatches"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("digest mismatch in 0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps.json"),
        "stderr should contain digest mismatch error; got: {stderr}"
    );
}

#[test]
fn test_verify_only_ignores_unselected_lockfile_entries() {
    // A lockfile entry for an entry that was not selected, and that the catalogue does not
    // produce, must not be reported as missing when `--only` selects a different entry.
    let tiny = "0016x0016-1-30fps-gradient-disc-fixed-raw-iso-rot180-scaled-zero-fps";
    let lock_content =
        fs::read_to_string(catalog_lock_path()).expect("failed to read catalog.lock");
    let bogus_digest = "1".repeat(64);
    let extended = format!(
        "{lock_content}bogus-entry.json {bogus_digest} 10\nbogus-entry-media.mp4 {bogus_digest} 20\n"
    );
    let temp_lock = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-scope.lock");
    fs::write(&temp_lock, extended).expect("failed to write temp lockfile");

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--only")
        .arg(tiny)
        .arg("--lock")
        .arg(&temp_lock)
        .output()
        .expect("failed to run synthvid-cli");

    drop(fs::remove_file(&temp_lock));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "an unselected extra lockfile entry must not fail verify; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("bogus-entry"),
        "unselected entries must not be reported; stderr: {stderr}"
    );
}

#[test]
fn test_verify_corpus_without_a_lockfile_names_the_problem() {
    let corpus = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-no-corpus");
    drop(fs::remove_dir_all(&corpus));

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .expect("failed to run synthvid-cli");

    assert!(!output.status.success(), "a missing corpus must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read lockfile"),
        "stderr should name the unreadable lockfile; got: {stderr}"
    );
}

#[test]
fn test_verify_corpus_reports_a_lockfile_entry_with_no_file() {
    let corpus = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("synthvid-verify-empty-corpus");
    drop(fs::remove_dir_all(&corpus));
    fs::create_dir_all(&corpus).expect("failed to create corpus directory");
    let digest = "0".repeat(64);
    fs::write(
        corpus.join("catalog.lock"),
        format!("only-entry.json {digest} 1\n"),
    )
    .expect("failed to write corpus lockfile");

    let output = Command::new(synthvid_bin())
        .arg("verify")
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .expect("failed to run synthvid-cli");

    drop(fs::remove_dir_all(&corpus));

    assert!(!output.status.success(), "a missing file must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("only-entry.json"),
        "stderr should name the missing file; got: {stderr}"
    );
}
