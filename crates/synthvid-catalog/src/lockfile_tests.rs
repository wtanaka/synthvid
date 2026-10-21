//! Tests for lockfile format and verification.

#[cfg(test)]
mod lockfile_test_cases {
    use super::super::*;
    use crate::sha256::sha256;
    use synthvid_testkit::{ensure, ensure_eq, Fail, OrFail as _};

    /// A corpus artifact named `name` holding `bytes`.
    fn artifact(name: &LockfileName, bytes: ArtifactBytes) -> CorpusArtifact {
        CorpusArtifact {
            name: RawName(name.as_str().to_owned()),
            bytes,
        }
    }

    fn check_lockfile_name_valid() -> Result<(), Fail> {
        ensure_eq!(
            LockfileName::new("test-0.json").or_fail()?.as_str(),
            "test-0.json"
        );
        ensure_eq!(
            LockfileName::new(
                "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef.json"
            )
            .or_fail()?
            .as_str(),
            "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef.json"
        );
        ensure_eq!(
            LockfileName::new(
                "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef-media.mp4"
            )
            .or_fail()?
            .as_str(),
            "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef-media.mp4"
        );
        Ok(())
    }

    #[test]
    fn lockfile_name_valid() {
        assert_eq!(check_lockfile_name_valid(), Ok(()));
    }

    #[test]
    fn lockfile_name_invalid() {
        assert!(LockfileName::new("noextension").is_err(), "no dot rejected");
        assert!(
            LockfileName::new("multiple.dots.here.json").is_err(),
            "multiple dots rejected"
        );
        assert!(LockfileName::new(".json").is_err(), "empty stem rejected");
        assert!(
            LockfileName::new("test.").is_err(),
            "empty extension rejected"
        );
        assert!(
            LockfileName::new("Test.json").is_err(),
            "uppercase rejected"
        );
        assert!(
            LockfileName::new("test_file.json").is_err(),
            "underscore rejected"
        );
        assert!(
            LockfileName::new("test.JSON").is_err(),
            "uppercase extension rejected"
        );
        assert!(LockfileName::new("").is_err(), "empty rejected");
    }

    #[test]
    fn content_length_valid() {
        assert_eq!(ContentLength::new(1).get(), 1);
        assert_eq!(ContentLength::new(1000).get(), 1000);
        assert_eq!(ContentLength::new(u64::MAX).get(), u64::MAX);
    }

    #[test]
    fn content_length_zero_accepted() {
        assert_eq!(ContentLength::new(0).get(), 0, "zero length is accepted");
    }

    fn check_lockfile_entry_serialization() -> Result<(), Fail> {
        let name = LockfileName::new("test.json").or_fail()?;
        let digest = Digest::new([0x01; 32]);
        let length = ContentLength::new(42);
        let entry = LockfileEntry::new(name, digest, length);

        let line = entry.to_string();
        ensure!(line.ends_with('\n'), "line must end with newline");
        ensure!(
            line.contains("test.json"),
            "line must contain artifact name"
        );
        ensure!(line.contains("01010101"), "line must contain digest");
        ensure!(line.contains("42"), "line must contain length");
        Ok(())
    }

    #[test]
    fn lockfile_entry_serialization() {
        assert_eq!(check_lockfile_entry_serialization(), Ok(()));
    }

    fn check_generate_lockfile_sorted() -> Result<(), Fail> {
        // Create entries in non-sorted order
        let z_name = LockfileName::new("z-file.json").or_fail()?;
        let z_digest = Digest::new([0x01; 32]);
        let z_entry = LockfileEntry::new(z_name, z_digest, ContentLength::new(10));

        let a_name = LockfileName::new("a-file.json").or_fail()?;
        let a_digest = Digest::new([0x02; 32]);
        let a_entry = LockfileEntry::new(a_name, a_digest, ContentLength::new(20));

        let m_name = LockfileName::new("m-file.json").or_fail()?;
        let m_digest = Digest::new([0x03; 32]);
        let m_entry = LockfileEntry::new(m_name, m_digest, ContentLength::new(30));

        let lockfile = generate_lockfile(vec![z_entry, a_entry, m_entry]);

        let lines: Vec<&str> = lockfile.0.lines().collect();
        ensure_eq!(lines.len(), 3);
        ensure!(lines.first().or_fail()?.starts_with("a-file.json"));
        ensure!(lines.get(1).or_fail()?.starts_with("m-file.json"));
        ensure!(lines.get(2).or_fail()?.starts_with("z-file.json"));
        Ok(())
    }

    #[test]
    fn generate_lockfile_sorted() {
        assert_eq!(check_generate_lockfile_sorted(), Ok(()));
    }

    fn check_parse_lockfile_valid() -> Result<(), Fail> {
        let content = LockfileText("a-file.json 0101010101010101010101010101010101010101010101010101010101010101 42\nm-file.json 0202020202020202020202020202020202020202020202020202020202020202 100\nz-file.json 0303030303030303030303030303030303030303030303030303030303030303 1000\n".to_owned());

        let entries = parse_lockfile(&content).or_fail()?;
        ensure_eq!(entries.len(), 3);

        let first = entries.first().or_fail()?;
        ensure_eq!(first.name.as_str(), "a-file.json");
        ensure_eq!(first.length.get(), 42);

        let second = entries.get(1).or_fail()?;
        ensure_eq!(second.name.as_str(), "m-file.json");
        ensure_eq!(second.length.get(), 100);

        let third = entries.get(2).or_fail()?;
        ensure_eq!(third.name.as_str(), "z-file.json");
        ensure_eq!(third.length.get(), 1000);
        Ok(())
    }

    #[test]
    fn parse_lockfile_valid() {
        assert_eq!(check_parse_lockfile_valid(), Ok(()));
    }

    fn check_verify_matching_corpus() -> Result<(), Fail> {
        // Create a lockfile entry
        let artifact_name = LockfileName::new("test.txt").or_fail()?;
        let content = b"hello";
        let digest = sha256(content);
        let length = ContentLength::new(u64::try_from(content.len()).or_fail()?);
        let entry = LockfileEntry::new(artifact_name.clone(), digest, length);

        // Create a corpus with matching content
        let corpus = vec![artifact(&artifact_name, ArtifactBytes(content.to_vec()))];

        let diffs = verify(&[entry], corpus);
        ensure_eq!(diffs.len(), 0, "matching corpus should have no differences");
        Ok(())
    }

    #[test]
    fn verify_matching_corpus() {
        assert_eq!(check_verify_matching_corpus(), Ok(()));
    }

    fn check_verify_missing_manifest() -> Result<(), Fail> {
        // Create a lockfile with a manifest entry
        let manifest_name = LockfileName::new("test.json").or_fail()?;
        let digest = Digest::new([0x01; 32]);
        let entry = LockfileEntry::new(manifest_name.clone(), digest, ContentLength::new(100));

        // Create a corpus with only the media file, not the manifest
        let media_name = LockfileName::new("test-media.mp4").or_fail()?;
        let media_digest = Digest::new([0x02; 32]);
        let media_entry =
            LockfileEntry::new(media_name.clone(), media_digest, ContentLength::new(5000));

        let corpus = vec![artifact(&media_name, ArtifactBytes(vec![0_u8; 5000]))];

        let diffs = verify(&[entry, media_entry], corpus);
        ensure!(
            diffs.contains(&LockfileDifference::Missing {
                name: manifest_name
            }),
            "should report missing manifest from corpus"
        );
        Ok(())
    }

    #[test]
    fn verify_missing_manifest() {
        assert_eq!(check_verify_missing_manifest(), Ok(()));
    }

    fn check_verify_flipped_byte() -> Result<(), Fail> {
        let content = b"hello world";
        let artifact_name = LockfileName::new("test.txt").or_fail()?;
        let correct_digest = sha256(content);
        let entry = LockfileEntry::new(
            artifact_name.clone(),
            correct_digest,
            ContentLength::new(u64::try_from(content.len()).or_fail()?),
        );

        // Create corpus with one byte flipped: 'h' (0x68) XOR 0xFF is 0x97.
        let corrupted = b"\x97ello world".to_vec();
        let corrupted_digest = sha256(&corrupted);

        let corpus = vec![artifact(&artifact_name, ArtifactBytes(corrupted))];

        let diffs = verify(&[entry], corpus);
        ensure_eq!(
            diffs,
            vec![LockfileDifference::DigestMismatch {
                name: artifact_name,
                expected: correct_digest,
                actual: corrupted_digest,
            }],
            "should report digest mismatch for the correct artifact"
        );
        Ok(())
    }

    #[test]
    fn verify_flipped_byte() {
        assert_eq!(check_verify_flipped_byte(), Ok(()));
    }

    fn check_verify_reports_all_differences() -> Result<(), Fail> {
        // Create lockfile with multiple entries
        let name1 = LockfileName::new("file1.json").or_fail()?;
        let name2 = LockfileName::new("file2.json").or_fail()?;
        let name3 = LockfileName::new("file3.json").or_fail()?;
        let name4 = LockfileName::new("file4.json").or_fail()?;

        let digest1 = Digest::new([0x01; 32]);
        let digest2 = Digest::new([0x02; 32]);
        let digest3 = Digest::new([0x03; 32]);

        let entry1 = LockfileEntry::new(name1.clone(), digest1, ContentLength::new(100));
        let entry2 = LockfileEntry::new(name2.clone(), digest2, ContentLength::new(200));
        let entry3 = LockfileEntry::new(name3.clone(), digest3, ContentLength::new(300));

        // Create corpus that differs from lockfile in multiple ways
        // file1: missing from corpus (in lockfile)
        // file2: content changed (digest different)
        // file3: missing from corpus (in lockfile)
        // file4: extra in corpus (not in lockfile)
        let changed = vec![0xff; 200];
        let changed_digest = sha256(&changed);
        let corpus = vec![
            artifact(&name2, ArtifactBytes(changed)), // Different digest
            artifact(&name4, ArtifactBytes(vec![0x42; 50])), // Extra file
        ];

        let diffs = verify(&[entry1, entry2, entry3], corpus);

        // Should report: digest mismatch for file2 and extra file4, in corpus
        // order, then missing file1 and file3, in lockfile order.
        ensure_eq!(
            diffs,
            vec![
                LockfileDifference::DigestMismatch {
                    name: name2,
                    expected: digest2,
                    actual: changed_digest,
                },
                LockfileDifference::Extra { name: name4 },
                LockfileDifference::Missing { name: name1 },
                LockfileDifference::Missing { name: name3 },
            ],
            "should report all differences"
        );
        Ok(())
    }

    #[test]
    fn verify_reports_all_differences() {
        assert_eq!(check_verify_reports_all_differences(), Ok(()));
    }

    fn check_verify_artifact_in_lockfile_missing_from_corpus() -> Result<(), Fail> {
        // Build a lockfile entry for an artifact
        let artifact_name = LockfileName::new("artifact.json").or_fail()?;
        let digest = Digest::new([0xaa; 32]);
        let length = ContentLength::new(512);
        let lockfile_entry = LockfileEntry::new(artifact_name.clone(), digest, length);

        // Create a corpus that lacks this artifact
        let corpus: Vec<CorpusArtifact> = vec![];

        let diffs = verify(&[lockfile_entry], corpus);

        // Should report a single Missing difference for this artifact
        ensure_eq!(
            diffs,
            vec![LockfileDifference::Missing {
                name: artifact_name
            }],
            "should report Missing (not Extra) for an artifact in lockfile but absent from corpus"
        );
        Ok(())
    }

    #[test]
    fn verify_artifact_in_lockfile_missing_from_corpus() {
        assert_eq!(
            check_verify_artifact_in_lockfile_missing_from_corpus(),
            Ok(())
        );
    }
}
