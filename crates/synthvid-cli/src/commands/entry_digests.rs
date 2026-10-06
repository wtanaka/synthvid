//! Shared typed digest helper for computing and comparing entry digests.

use super::entry_name::{EntryFiles, EntryFilesError};
use core::fmt;
use synthvid_catalog::{
    cover::Entry, generate_entry, sha256, ContentLength, GenerationError, LockfileEntry,
};

/// The computed digest values for a single catalogue entry.
pub(super) struct EntryDigests {
    /// The manifest file lockfile entry.
    pub(super) manifest: LockfileEntry,
    /// The media file lockfile entry.
    pub(super) media: LockfileEntry,
}

/// Why an entry's digests could not be computed. Each variant exposes its cause only
/// through `source`, so printing the chain shows each cause once.
#[derive(Debug)]
pub(super) enum EntryDigestError {
    /// Entry generation failed.
    Generation(GenerationError),
    /// A file name of the entry is not a valid lockfile name.
    Files(EntryFilesError),
    /// Manifest too large to fit in a u64 length.
    ManifestLength(core::num::TryFromIntError),
    /// Media too large to fit in a u64 length.
    MediaLength(core::num::TryFromIntError),
}

impl fmt::Display for EntryDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generation(_) => f.write_str("entry generation failed"),
            Self::Files(_) => f.write_str("entry has an invalid file name"),
            Self::ManifestLength(_) => f.write_str("manifest length does not fit in 64 bits"),
            Self::MediaLength(_) => f.write_str("media length does not fit in 64 bits"),
        }
    }
}

impl core::error::Error for EntryDigestError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Generation(e) => Some(e),
            Self::Files(e) => Some(e),
            Self::ManifestLength(e) | Self::MediaLength(e) => Some(e),
        }
    }
}

/// Computes the digest and length entries for a catalogue entry.
pub(super) fn entry_digests(entry: &Entry) -> Result<EntryDigests, EntryDigestError> {
    let files = EntryFiles::of(entry).map_err(EntryDigestError::Files)?;
    let generated = generate_entry(entry).map_err(EntryDigestError::Generation)?;

    let manifest_length = ContentLength::new(
        u64::try_from(generated.manifest_json().len()).map_err(EntryDigestError::ManifestLength)?,
    );
    let media_length = ContentLength::new(
        u64::try_from(generated.media_bytes().len()).map_err(EntryDigestError::MediaLength)?,
    );

    Ok(EntryDigests {
        manifest: LockfileEntry::new(
            files.manifest,
            sha256(generated.manifest_json().as_bytes()),
            manifest_length,
        ),
        media: LockfileEntry::new(files.media, sha256(generated.media_bytes()), media_length),
    })
}
