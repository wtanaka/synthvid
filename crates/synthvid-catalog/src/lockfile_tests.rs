//! Tests for lockfile format and verification.

#[cfg(test)]
mod lockfile_test_cases {
    use super::super::*;

    fn make_lockfile_name(s: &str) -> LockfileName {
        LockfileName::new(s).expect("test name must be valid")
    }

    fn make_content_length(len: u64) -> ContentLength {
        ContentLength::new(len)
    }

    #[test]
    fn test_lockfile_name_valid() {
        assert_eq!(make_lockfile_name("test-0.json").as_str(), "test-0.json");
        assert_eq!(
            make_lockfile_name(
                "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef.json"
            )
            .as_str(),
            "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef.json"
        );
        assert_eq!(
            make_lockfile_name(
                "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef-media.mp4"
            )
            .as_str(),
            "0256x0256-1-30fps-solid-disc-fixed-raw-iso-identity-unscaled-nodef-media.mp4"
        );
    }

    #[test]
    fn test_lockfile_name_invalid() {
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
    fn test_content_length_valid() {
        assert_eq!(make_content_length(1).get(), 1);
        assert_eq!(make_content_length(1000).get(), 1000);
        assert_eq!(make_content_length(u64::MAX).get(), u64::MAX);
    }

    #[test]
    fn test_content_length_zero_accepted() {
        assert_eq!(ContentLength::new(0).get(), 0, "zero length is accepted");
    }

    #[test]
    fn test_lockfile_entry_serialization() {
        let name = make_lockfile_name("test.json");
        let digest = Digest::new([0x01; 32]);
        let length = make_content_length(42);
        let entry = LockfileEntry::new(name, digest, length);

        let line = entry.to_line();
        assert!(line.ends_with('\n'), "line must end with newline");
        assert!(
            line.contains("test.json"),
            "line must contain artifact name"
        );
        assert!(line.contains("01010101"), "line must contain digest");
        assert!(line.contains("42"), "line must contain length");
    }

    #[test]
    fn test_generate_lockfile_sorted() {
        // Create entries in non-sorted order
        let z_name = make_lockfile_name("z-file.json");
        let z_digest = Digest::new([0x01; 32]);
        let z_entry = LockfileEntry::new(z_name, z_digest, make_content_length(10));

        let a_name = make_lockfile_name("a-file.json");
        let a_digest = Digest::new([0x02; 32]);
        let a_entry = LockfileEntry::new(a_name, a_digest, make_content_length(20));

        let m_name = make_lockfile_name("m-file.json");
        let m_digest = Digest::new([0x03; 32]);
        let m_entry = LockfileEntry::new(m_name, m_digest, make_content_length(30));

        let lockfile = generate_lockfile(vec![z_entry, a_entry, m_entry]);

        let lines: Vec<&str> = lockfile.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("a-file.json"));
        assert!(lines[1].starts_with("m-file.json"));
        assert!(lines[2].starts_with("z-file.json"));
    }

    #[test]
    fn test_parse_lockfile_valid() {
        let content = "a-file.json 0101010101010101010101010101010101010101010101010101010101010101 42\nm-file.json 0202020202020202020202020202020202020202020202020202020202020202 100\nz-file.json 0303030303030303030303030303030303030303030303030303030303030303 1000\n";

        let entries = parse_lockfile(content).expect("parsing should succeed");
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].name.as_str(), "a-file.json");
        assert_eq!(entries[0].length.get(), 42);

        assert_eq!(entries[1].name.as_str(), "m-file.json");
        assert_eq!(entries[1].length.get(), 100);

        assert_eq!(entries[2].name.as_str(), "z-file.json");
        assert_eq!(entries[2].length.get(), 1000);
    }

    #[test]
    fn test_verify_matching_corpus() {
        // Create a lockfile entry
        let artifact_name = make_lockfile_name("test.txt");
        let content = b"hello";
        let digest = crate::sha256::sha256(content);
        let length = make_content_length(content.len().try_into().expect("test content too large"));
        let entry = LockfileEntry::new(artifact_name, digest, length);

        // Create a corpus with matching content
        let corpus = vec![("test.txt".to_owned(), content.to_vec())];

        let diffs = verify(&[entry], corpus);
        assert_eq!(diffs.len(), 0, "matching corpus should have no differences");
    }

    #[test]
    fn test_verify_missing_manifest() {
        // Create a lockfile with a manifest entry
        let manifest_name = make_lockfile_name("test.json");
        let digest = Digest::new([0x01; 32]);
        let entry = LockfileEntry::new(manifest_name, digest, make_content_length(100));

        // Create a corpus with only the media file, not the manifest
        let media_name = make_lockfile_name("test-media.mp4");
        let media_digest = Digest::new([0x02; 32]);
        let media_entry = LockfileEntry::new(media_name, media_digest, make_content_length(5000));

        // Parse to get expected values
        let corpus = vec![("test-media.mp4".to_owned(), vec![0u8; 5000])];

        let diffs = verify(&[entry, media_entry], corpus);
        assert!(
            diffs.iter().any(
                |d| matches!(d, LockfileDifference::Missing { name } if name.as_str() == "test.json")
            ),
            "should report missing manifest from corpus"
        );
    }

    #[test]
    fn test_verify_flipped_byte() {
        let content = b"hello world";
        let artifact_name = make_lockfile_name("test.txt");
        let correct_digest = crate::sha256::sha256(content);
        let entry = LockfileEntry::new(
            artifact_name,
            correct_digest,
            make_content_length(content.len().try_into().expect("test content too large")),
        );

        // Create corpus with one byte flipped
        let mut corrupted = content.to_vec();
        corrupted[0] ^= 0xFF;

        let corpus = vec![("test.txt".to_owned(), corrupted)];

        let diffs = verify(&[entry], corpus);
        assert_eq!(diffs.len(), 1);
        assert!(
            matches!(
                &diffs[0],
                LockfileDifference::DigestMismatch { name, .. } if name.as_str() == "test.txt"
            ),
            "should report digest mismatch for the correct artifact"
        );
    }

    #[test]
    fn test_verify_reports_all_differences() {
        // Create lockfile with multiple entries
        let name1 = make_lockfile_name("file1.json");
        let name2 = make_lockfile_name("file2.json");
        let name3 = make_lockfile_name("file3.json");

        let digest1 = Digest::new([0x01; 32]);
        let digest2 = Digest::new([0x02; 32]);
        let digest3 = Digest::new([0x03; 32]);

        let entry1 = LockfileEntry::new(name1, digest1, make_content_length(100));
        let entry2 = LockfileEntry::new(name2, digest2, make_content_length(200));
        let entry3 = LockfileEntry::new(name3, digest3, make_content_length(300));

        // Create corpus that differs from lockfile in multiple ways
        // file1: missing from corpus (in lockfile)
        // file2: content changed (digest different)
        // file3: missing from corpus (in lockfile)
        // file4: extra in corpus (not in lockfile)
        let corpus = vec![
            ("file2.json".to_owned(), vec![0xff; 200]), // Different digest
            ("file4.json".to_owned(), vec![0x42; 50]),  // Extra file
        ];

        let diffs = verify(&[entry1, entry2, entry3], corpus);

        // Should report: missing file1, digest mismatch for file2, missing file3, and extra file4
        assert_eq!(diffs.len(), 4, "should report all differences");

        let has_missing_file1 = diffs.iter().any(
            |d| matches!(d, LockfileDifference::Missing { name } if name.as_str() == "file1.json"),
        );
        let has_digest_mismatch_file2 = diffs.iter().any(
            |d| matches!(d, LockfileDifference::DigestMismatch { name, .. } if name.as_str() == "file2.json"),
        );
        let has_missing_file3 = diffs.iter().any(
            |d| matches!(d, LockfileDifference::Missing { name } if name.as_str() == "file3.json"),
        );
        let has_extra_file4 = diffs.iter().any(
            |d| matches!(d, LockfileDifference::Extra { name } if name.as_str() == "file4.json"),
        );

        assert!(
            has_missing_file1,
            "should report file1 as missing from corpus"
        );
        assert!(
            has_digest_mismatch_file2,
            "should report digest mismatch for file2"
        );
        assert!(
            has_missing_file3,
            "should report file3 as missing from corpus"
        );
        assert!(has_extra_file4, "should report file4 as extra in corpus");
    }

    #[test]
    fn test_verify_artifact_in_lockfile_missing_from_corpus() {
        // Build a lockfile entry for an artifact
        let artifact_name = make_lockfile_name("artifact.json");
        let digest = Digest::new([0xaa; 32]);
        let length = make_content_length(512);
        let lockfile_entry = LockfileEntry::new(artifact_name, digest, length);

        // Create a corpus that lacks this artifact
        let corpus: Vec<(String, Vec<u8>)> = vec![];

        let diffs = verify(&[lockfile_entry], corpus);

        // Should report a single Missing difference for this artifact
        assert_eq!(diffs.len(), 1, "should report exactly one difference");
        assert!(
            matches!(
                &diffs[0],
                LockfileDifference::Missing { name } if name.as_str() == "artifact.json"
            ),
            "should report Missing (not Extra) for an artifact in lockfile but absent from corpus"
        );
    }
}
