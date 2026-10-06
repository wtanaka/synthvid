//! Entry names, their file names, and the selection of entries a verification covers.

use core::fmt::{Display, Formatter, Result as FmtResult};
use std::collections::BTreeSet;
use synthvid_catalog::cover::Entry;
use synthvid_catalog::{LockfileEntry, LockfileName, LockfileNameError};

/// The name of a catalogue entry, shared by its manifest and media files.
///
/// Built only from a catalogue entry, from a lockfile name, or from a name the
/// lockfile contains, so a value always names something the catalogue or the
/// lockfile holds.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct EntryName(String);

impl EntryName {
    /// The entry a lockfile name belongs to: `NAME.json` and `NAME-media.EXT` both belong to `NAME`.
    pub(super) fn of_lockfile_name(name: &LockfileName) -> Option<Self> {
        let text = name.as_str();
        if let Some(entry) = text.strip_suffix(".json") {
            return Some(Self(entry.to_owned()));
        }
        text.rfind("-media.")
            .and_then(|index| text.get(..index))
            .map(|entry| Self(entry.to_owned()))
    }

    /// The name of a catalogue entry.
    pub(super) fn of_entry(entry: &Entry) -> Self {
        Self(entry.manifest_name().as_str().to_owned())
    }
}

impl Display for EntryName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

/// An entry name as typed on the command line, before it is checked against a lockfile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RequestedName(String);

impl RequestedName {
    /// Wraps a name exactly as the user typed it.
    pub(super) const fn new(typed: String) -> Self {
        Self(typed)
    }
}

impl Display for RequestedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

/// A requested entry name the lockfile does not contain.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct UnknownEntryName {
    /// The name that was asked for.
    pub(super) name: RequestedName,
}

impl Display for UnknownEntryName {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "unknown entry name: {}", self.name)
    }
}

/// The two file names an entry produces.
pub(super) struct EntryFiles {
    /// The manifest file name, `NAME.json`.
    pub(super) manifest: LockfileName,
    /// The media file name, `NAME-media.EXT`.
    pub(super) media: LockfileName,
}

/// An entry's file name that is not a valid lockfile name.
#[derive(Debug)]
pub(super) enum EntryFilesError {
    /// The manifest file name is invalid.
    Manifest(LockfileNameError),
    /// The media file name is invalid.
    Media(LockfileNameError),
}

impl Display for EntryFilesError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Manifest(_) => f.write_str("invalid manifest file name"),
            Self::Media(_) => f.write_str("invalid media file name"),
        }
    }
}

impl core::error::Error for EntryFilesError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Manifest(e) | Self::Media(e) => Some(e),
        }
    }
}

impl EntryFiles {
    /// The file names a catalogue entry produces. The one place they are spelled out.
    pub(super) fn of(entry: &Entry) -> Result<Self, EntryFilesError> {
        let manifest = LockfileName::new(format!("{}.json", entry.manifest_name().as_str()))
            .map_err(EntryFilesError::Manifest)?;
        let media = LockfileName::new(entry.media_name()).map_err(EntryFilesError::Media)?;
        Ok(Self { manifest, media })
    }
}

/// Entry names asked for on the command line, not yet checked against a lockfile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Requested {
    /// No `--only`: every entry.
    Everything,
    /// One or more `--only NAME` arguments, as typed.
    Named(Vec<RequestedName>),
}

/// The entries a verification covers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EntrySelection {
    /// Every entry, and every lockfile line.
    All,
    /// Exactly these entries, each known to the lockfile.
    Only(BTreeSet<EntryName>),
}

impl EntrySelection {
    /// Checks the requested names against the lockfile's entries.
    pub(super) fn resolve(
        requested: &Requested,
        lockfile: &[LockfileEntry],
    ) -> Result<Self, UnknownEntryName> {
        match requested {
            Requested::Everything => Ok(Self::All),
            Requested::Named(names) => {
                let known: BTreeSet<EntryName> = lockfile
                    .iter()
                    .filter_map(|entry| EntryName::of_lockfile_name(&entry.name))
                    .collect();
                let mut selected = BTreeSet::new();
                for name in names {
                    let Some(found) = known.iter().find(|entry| entry.0 == name.0) else {
                        return Err(UnknownEntryName { name: name.clone() });
                    };
                    selected.insert(found.clone());
                }
                Ok(Self::Only(selected))
            }
        }
    }

    /// Whether the selection covers the named entry.
    pub(super) fn covers_entry(&self, name: &EntryName) -> bool {
        match self {
            Self::All => true,
            Self::Only(selected) => selected.contains(name),
        }
    }

    /// Whether the selection covers a lockfile line. `All` covers every line, including
    /// names that belong to no entry; `Only` covers the lines of the selected entries.
    pub(super) fn covers_file(&self, name: &LockfileName) -> bool {
        match self {
            Self::All => true,
            Self::Only(selected) => {
                EntryName::of_lockfile_name(name).is_some_and(|entry| selected.contains(&entry))
            }
        }
    }
}
