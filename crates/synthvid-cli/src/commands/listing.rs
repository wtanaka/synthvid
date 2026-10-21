//! The list and manifest subcommands: argument parsing, and the reports they
//! compute.

use super::message::Message;
use super::report::{Flag, OutText, Report, Step, Word};
use super::{Argv, Outcome};
use synthvid_catalog::cover::generate_catalogue;
use synthvid_catalog::generate_entry;

/// The output format `list` was asked for.
enum ListFormat {
    /// One name per line.
    Text,
    /// A JSON array of names.
    Json,
    /// A format name that no output format carries.
    Unknown(Word),
}

/// Computes the report for the list subcommand.
pub(super) fn plan_list(args: Argv<'_>) -> Report {
    let mut format = ListFormat::Text;

    let mut args_iter = args.0.iter();
    while let Some(arg) = args_iter.next() {
        if arg.as_str() == "--format" {
            if let Some(value) = args_iter.next() {
                format = match value.as_str() {
                    "text" => ListFormat::Text,
                    "json" => ListFormat::Json,
                    other => ListFormat::Unknown(Word(other.to_owned())),
                };
            } else {
                return Report::failed(Message::MissingValue(Flag::Format));
            }
        } else {
            return Report::failed(Message::UnknownOption(Word(arg.clone())));
        }
    }

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(cause) => return Report::failed(Message::CatalogFailed(cause)),
    };

    match format {
        ListFormat::Text => {
            let mut text = String::new();
            for entry in &catalogue {
                text.push_str(entry.manifest_name().as_str());
                text.push('\n');
            }
            Report::new(vec![Step::Stdout(OutText(text))], Outcome::Success)
        }
        ListFormat::Json => {
            let names: Vec<String> = catalogue
                .iter()
                .map(|entry| format!("\"{}\"", entry.manifest_name().as_str()))
                .collect();
            let text = format!("[{}]\n", names.join(","));
            Report::new(vec![Step::Stdout(OutText(text))], Outcome::Success)
        }
        ListFormat::Unknown(name) => Report::failed(Message::UnknownFormat(name)),
    }
}

/// Computes the report for the manifest subcommand.
pub(super) fn plan_manifest(args: Argv<'_>) -> Report {
    let mut name_value: Option<Word> = None;

    let mut args_iter = args.0.iter();
    while let Some(arg) = args_iter.next() {
        if arg.as_str() == "--name" {
            if let Some(value) = args_iter.next() {
                name_value = Some(Word(value.clone()));
            } else {
                return Report::failed(Message::MissingValue(Flag::Name));
            }
        } else {
            return Report::failed(Message::UnknownOption(Word(arg.clone())));
        }
    }

    let Some(name) = name_value else {
        return Report::failed(Message::MissingFlag(Flag::Name));
    };

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(cause) => return Report::failed(Message::CatalogFailed(cause)),
    };

    for entry in catalogue {
        if entry.manifest_name().as_str() == name.0 {
            return match generate_entry(&entry) {
                Ok(generated) => Report::new(
                    vec![Step::ManifestStdout(OutText(
                        generated.manifest_json().as_str().to_owned(),
                    ))],
                    Outcome::Success,
                ),
                Err(cause) => Report::failed(Message::ManifestFailed(cause)),
            };
        }
    }

    Report::failed(Message::EntryNotFound(name))
}

#[cfg(test)]
mod tests;
