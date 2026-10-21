//! Shared typed digest helper for computing and comparing entry digests.

use super::entry_name::{EntryFiles, EntryFilesError};
use super::error_chain::ErrorChain;
use core::error::Error;
use core::fmt;
use core::num::TryFromIntError;
use synthvid_catalog::{
    cover::Entry, generate_entry, sha256, ContentLength, EntryError, LockfileEntry,
};

/// The computed digest values for a single catalogue entry.
pub(super) struct EntryDigests {
    /// The manifest file lockfile entry.
    pub(super) manifest: LockfileEntry,
    /// The media file lockfile entry.
    pub(super) media: LockfileEntry,
}

/// Why an entry's digests could not be computed. Its display names the failure and then
/// each cause once; it reports no separate `source`.
#[derive(Debug)]
pub(super) enum EntryDigestError {
    /// Entry generation failed.
    Generation(EntryError),
    /// A file name of the entry is not a valid lockfile name.
    Files(EntryFilesError),
    /// Manifest too large to fit in a u64 length.
    ManifestLength(TryFromIntError),
    /// Media too large to fit in a u64 length.
    MediaLength(TryFromIntError),
}

impl fmt::Display for EntryDigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generation(error) => {
                write!(formatter, "entry generation failed: {}", ErrorChain(error))
            }
            Self::Files(error) => write!(formatter, "entry has an invalid file name: {error}"),
            Self::ManifestLength(error) => write!(
                formatter,
                "manifest length does not fit in 64 bits: {}",
                ErrorChain(error)
            ),
            Self::MediaLength(error) => {
                write!(
                    formatter,
                    "media length does not fit in 64 bits: {}",
                    ErrorChain(error)
                )
            }
        }
    }
}

impl Error for EntryDigestError {}

/// Computes the digest and length entries for a catalogue entry.
pub(super) fn entry_digests(entry: &Entry) -> Result<EntryDigests, EntryDigestError> {
    let files = EntryFiles::of(entry).map_err(EntryDigestError::Files)?;
    let generated = generate_entry(entry).map_err(EntryDigestError::Generation)?;

    let manifest_length = ContentLength::new(
        u64::try_from(generated.manifest_json().as_str().len())
            .map_err(EntryDigestError::ManifestLength)?,
    );
    let media_length = ContentLength::new(
        u64::try_from(generated.media_bytes().as_bytes().len())
            .map_err(EntryDigestError::MediaLength)?,
    );

    Ok(EntryDigests {
        manifest: LockfileEntry::new(
            files.manifest,
            sha256(generated.manifest_json().as_str().as_bytes()),
            manifest_length,
        ),
        media: LockfileEntry::new(
            files.media,
            sha256(generated.media_bytes().as_bytes()),
            media_length,
        ),
    })
}
