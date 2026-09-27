//! Command implementations.

use std::fs;
use std::io::Write;
use std::path::Path;
use synthvid_catalog::{
    cover::generate_catalogue, generate_entry, generate_lockfile, parse_lockfile, sha256, verify,
    ContentLength, LockfileEntry, LockfileName,
};

/// Prints the usage message.
pub fn print_usage() {
    writeln!(std::io::stderr(), "usage: synthvid <command> [options]").ok();
    writeln!(std::io::stderr(), "commands:").ok();
    writeln!(
        std::io::stderr(),
        "  generate --out DIR [--only NAME]... [--dry-run]"
    )
    .ok();
    writeln!(std::io::stderr(), "  verify --corpus DIR").ok();
    writeln!(std::io::stderr(), "  list [--format text|json]").ok();
    writeln!(std::io::stderr(), "  manifest --name NAME").ok();
    writeln!(
        std::io::stderr(),
        "  lock --corpus DIR [--write] [--dry-run]"
    )
    .ok();
}

/// Runs the generate subcommand.
#[must_use]
pub fn cmd_generate(args: &[String]) -> i32 {
    match parse_generate_args(args) {
        Ok(parsed_args) => generate_impl(parsed_args),
        Err(code) => code,
    }
}

/// Parses arguments for the generate subcommand.
fn parse_generate_args(args: &[String]) -> Result<GenerateArgs, i32> {
    let mut out_dir = None;
    let mut only_names: Vec<String> = Vec::new();
    let mut dry_run = false;

    let mut args_iter = args.iter();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--out" => {
                if let Some(value) = args_iter.next() {
                    out_dir = Some(value.clone());
                } else {
                    writeln!(std::io::stderr(), "--out requires an argument").ok();
                    return Err(1);
                }
            }
            "--only" => {
                if let Some(value) = args_iter.next() {
                    only_names.push(value.clone());
                } else {
                    writeln!(std::io::stderr(), "--only requires an argument").ok();
                    return Err(1);
                }
            }
            "--dry-run" => {
                dry_run = true;
            }
            _ => {
                writeln!(std::io::stderr(), "unknown option: {arg}").ok();
                return Err(1);
            }
        }
    }

    let Some(out_dir) = out_dir else {
        writeln!(std::io::stderr(), "--out is required").ok();
        return Err(1);
    };

    Ok(GenerateArgs {
        out_dir,
        only_names,
        dry_run,
    })
}

/// Arguments for the generate subcommand.
struct GenerateArgs {
    /// Output directory path.
    out_dir: String,
    /// Names of entries to generate.
    only_names: Vec<String>,
    /// Whether to perform a dry run.
    dry_run: bool,
}

/// Implements the generate subcommand logic.
fn generate_impl(args: GenerateArgs) -> i32 {
    let out_dir = args.out_dir;
    let only_names = args.only_names;
    let dry_run = args.dry_run;

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to generate catalog: {e}").ok();
            return 1;
        }
    };

    let entries_to_generate: Vec<_> = if only_names.is_empty() {
        catalogue
    } else {
        catalogue
            .into_iter()
            .filter(|entry| {
                let manifest_name = entry.manifest_name();
                let name = manifest_name.as_str();
                only_names.iter().any(|n| n == name)
            })
            .collect()
    };

    if !dry_run {
        if let Err(e) = fs::create_dir_all(&out_dir) {
            writeln!(std::io::stderr(), "failed to create output directory: {e}").ok();
            return 1;
        }
    }

    let mut had_error = false;

    for entry in entries_to_generate {
        match generate_entry(&entry) {
            Ok(generated) => {
                if dry_run {
                    writeln!(
                        std::io::stderr(),
                        "would generate: {}",
                        entry.manifest_name().as_str()
                    )
                    .ok();
                } else {
                    let manifest_name = format!("{}.json", entry.manifest_name().as_str());
                    let media_name = entry.media_name();

                    let manifest_path = Path::new(&out_dir).join(&manifest_name);
                    let media_path = Path::new(&out_dir).join(&media_name);

                    let manifest_tmp = format!("{}.tmp", manifest_path.display());
                    let media_tmp = format!("{}.tmp", media_path.display());

                    match fs::write(&manifest_tmp, generated.manifest_json()) {
                        Ok(()) => {
                            if let Err(e) = fs::rename(&manifest_tmp, &manifest_path) {
                                writeln!(std::io::stderr(), "failed to rename manifest file: {e}")
                                    .ok();
                                drop(fs::remove_file(&manifest_tmp));
                                had_error = true;
                                continue;
                            }
                        }
                        Err(e) => {
                            writeln!(std::io::stderr(), "failed to write manifest: {e}").ok();
                            drop(fs::remove_file(&manifest_tmp));
                            had_error = true;
                            continue;
                        }
                    }

                    match fs::write(&media_tmp, generated.media_bytes()) {
                        Ok(()) => {
                            if let Err(e) = fs::rename(&media_tmp, &media_path) {
                                writeln!(std::io::stderr(), "failed to rename media file: {e}")
                                    .ok();
                                drop(fs::remove_file(&media_tmp));
                                had_error = true;
                                continue;
                            }
                        }
                        Err(e) => {
                            writeln!(std::io::stderr(), "failed to write media: {e}").ok();
                            drop(fs::remove_file(&media_tmp));
                            had_error = true;
                            continue;
                        }
                    }

                    writeln!(
                        std::io::stderr(),
                        "generated: {manifest_name} and {media_name}"
                    )
                    .ok();
                }
            }
            Err(e) => {
                writeln!(
                    std::io::stderr(),
                    "failed to generate {}: {}",
                    entry.manifest_name().as_str(),
                    e
                )
                .ok();
                had_error = true;
            }
        }
    }

    i32::from(had_error)
}

/// Runs the verify subcommand.
#[must_use]
pub fn cmd_verify(args: &[String]) -> i32 {
    let mut corpus_dir = None;

    let mut args_iter = args.iter();
    while let Some(arg) = args_iter.next() {
        if arg.as_str() == "--corpus" {
            if let Some(value) = args_iter.next() {
                corpus_dir = Some(value.clone());
            } else {
                writeln!(std::io::stderr(), "--corpus requires an argument").ok();
                return 1;
            }
        } else {
            writeln!(std::io::stderr(), "unknown option: {arg}").ok();
            return 1;
        }
    }

    let Some(corpus_dir) = corpus_dir else {
        writeln!(std::io::stderr(), "--corpus is required").ok();
        return 1;
    };

    let lockfile_path = Path::new(&corpus_dir).join("catalog.lock");
    let lockfile_content = match fs::read_to_string(&lockfile_path) {
        Ok(content) => content,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to read lockfile: {e}").ok();
            return 1;
        }
    };

    let lockfile_entries = match parse_lockfile(&lockfile_content) {
        Ok(entries) => entries,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to parse lockfile: {e}").ok();
            return 1;
        }
    };

    let mut corpus: Vec<(String, Vec<u8>)> = Vec::new();

    match fs::read_dir(&corpus_dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(dir_entry) => {
                        let path = dir_entry.path();
                        let filename = match path.file_name() {
                            Some(name) => name.to_string_lossy().to_string(),
                            None => continue,
                        };

                        if filename == "catalog.lock" {
                            continue;
                        }

                        match fs::read(&path) {
                            Ok(content) => {
                                corpus.push((filename, content));
                            }
                            Err(e) => {
                                writeln!(std::io::stderr(), "failed to read file {filename}: {e}")
                                    .ok();
                                return 1;
                            }
                        }
                    }
                    Err(e) => {
                        writeln!(std::io::stderr(), "failed to read directory entry: {e}").ok();
                        return 1;
                    }
                }
            }
        }
        Err(e) => {
            writeln!(std::io::stderr(), "failed to read corpus directory: {e}").ok();
            return 1;
        }
    }

    let differences = verify(&lockfile_entries, corpus);

    if differences.is_empty() {
        0
    } else {
        for diff in differences {
            writeln!(std::io::stderr(), "{diff}").ok();
        }
        1
    }
}

/// Runs the list subcommand.
#[must_use]
pub fn cmd_list(args: &[String]) -> i32 {
    let mut format = "text".to_owned();

    let mut args_iter = args.iter();
    while let Some(arg) = args_iter.next() {
        if arg.as_str() == "--format" {
            if let Some(value) = args_iter.next() {
                format.clone_from(value);
            } else {
                writeln!(std::io::stderr(), "--format requires an argument").ok();
                return 1;
            }
        } else {
            writeln!(std::io::stderr(), "unknown option: {arg}").ok();
            return 1;
        }
    }

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to generate catalog: {e}").ok();
            return 1;
        }
    };

    match format.as_str() {
        "text" => {
            for entry in catalogue {
                writeln!(std::io::stdout(), "{}", entry.manifest_name().as_str()).ok();
            }
        }
        "json" => {
            write!(std::io::stdout(), "[").ok();
            for (idx, entry) in catalogue.iter().enumerate() {
                if idx > 0 {
                    write!(std::io::stdout(), ",").ok();
                }
                write!(std::io::stdout(), "\"{}\"", entry.manifest_name().as_str()).ok();
            }
            writeln!(std::io::stdout(), "]").ok();
        }
        _ => {
            writeln!(std::io::stderr(), "unknown format: {format}").ok();
            return 1;
        }
    }

    0
}

/// Runs the manifest subcommand.
#[must_use]
pub fn cmd_manifest(args: &[String]) -> i32 {
    let mut name = None;

    let mut args_iter = args.iter();
    while let Some(arg) = args_iter.next() {
        if arg.as_str() == "--name" {
            if let Some(value) = args_iter.next() {
                name = Some(value.clone());
            } else {
                writeln!(std::io::stderr(), "--name requires an argument").ok();
                return 1;
            }
        } else {
            writeln!(std::io::stderr(), "unknown option: {arg}").ok();
            return 1;
        }
    }

    let Some(name) = name else {
        writeln!(std::io::stderr(), "--name is required").ok();
        return 1;
    };

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to generate catalog: {e}").ok();
            return 1;
        }
    };

    for entry in catalogue {
        if entry.manifest_name().as_str() == name {
            match generate_entry(&entry) {
                Ok(generated) => {
                    if let Err(e) =
                        std::io::stdout().write_all(generated.manifest_json().as_bytes())
                    {
                        writeln!(std::io::stderr(), "failed to write manifest: {e}").ok();
                        return 1;
                    }
                    return 0;
                }
                Err(e) => {
                    writeln!(std::io::stderr(), "failed to generate manifest: {e}").ok();
                    return 1;
                }
            }
        }
    }

    writeln!(std::io::stderr(), "entry not found: {name}").ok();
    1
}

/// Runs the lock subcommand.
#[must_use]
pub fn cmd_lock(args: &[String]) -> i32 {
    let parsed_args = match parse_lock_args(args) {
        Ok(a) => a,
        Err(code) => return code,
    };
    lock_impl(parsed_args)
}

/// Arguments for the lock subcommand.
struct LockArgs {
    /// Corpus directory path.
    corpus_dir: String,
    /// Whether to write the lockfile.
    write: bool,
    /// Whether to perform a dry run.
    dry_run: bool,
}

/// Parses arguments for the lock subcommand.
fn parse_lock_args(args: &[String]) -> Result<LockArgs, i32> {
    let mut corpus_dir = None;
    let mut write = false;
    let mut dry_run = false;

    let mut args_iter = args.iter();
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--corpus" => {
                if let Some(value) = args_iter.next() {
                    corpus_dir = Some(value.clone());
                } else {
                    writeln!(std::io::stderr(), "--corpus requires an argument").ok();
                    return Err(1);
                }
            }
            "--write" => {
                write = true;
            }
            "--dry-run" => {
                dry_run = true;
            }
            _ => {
                writeln!(std::io::stderr(), "unknown option: {arg}").ok();
                return Err(1);
            }
        }
    }

    let Some(corpus_dir) = corpus_dir else {
        writeln!(std::io::stderr(), "--corpus is required").ok();
        return Err(1);
    };

    Ok(LockArgs {
        corpus_dir,
        write,
        dry_run,
    })
}

/// Implements the lock subcommand logic.
fn lock_impl(args: LockArgs) -> i32 {
    let corpus_dir = args.corpus_dir;
    let write = args.write;
    let dry_run = args.dry_run;

    let catalogue = match generate_catalogue() {
        Ok(cat) => cat,
        Err(e) => {
            writeln!(std::io::stderr(), "failed to generate catalog: {e}").ok();
            return 1;
        }
    };

    let mut lockfile_entries = Vec::new();
    for entry in catalogue {
        match generate_entry(&entry) {
            Ok(generated) => {
                let manifest_name = format!("{}.json", entry.manifest_name().as_str());
                let media_name = entry.media_name();

                let manifest_digest = sha256(generated.manifest_json().as_bytes());
                let Ok(manifest_len) = u64::try_from(generated.manifest_json().len()) else {
                    writeln!(std::io::stderr(), "manifest too large").ok();
                    return 1;
                };
                let manifest_length = ContentLength::new(manifest_len);

                let media_digest = sha256(generated.media_bytes());
                let Ok(media_len) = u64::try_from(generated.media_bytes().len()) else {
                    writeln!(std::io::stderr(), "media too large").ok();
                    return 1;
                };
                let media_length = ContentLength::new(media_len);

                if let Ok(mname) = LockfileName::new(&manifest_name) {
                    lockfile_entries.push(LockfileEntry::new(
                        mname,
                        manifest_digest,
                        manifest_length,
                    ));
                }
                if let Ok(mname) = LockfileName::new(&media_name) {
                    lockfile_entries.push(LockfileEntry::new(mname, media_digest, media_length));
                }
            }
            Err(e) => {
                writeln!(
                    std::io::stderr(),
                    "failed to generate {}: {}",
                    entry.manifest_name().as_str(),
                    e
                )
                .ok();
                return 1;
            }
        }
    }

    let lockfile_content = generate_lockfile(lockfile_entries);

    if write {
        if !dry_run {
            let lockfile_path = Path::new(&corpus_dir).join("catalog.lock");
            let tmp_path = format!("{}.tmp", lockfile_path.display());

            match fs::write(&tmp_path, &lockfile_content) {
                Ok(()) => {
                    if let Err(e) = fs::rename(&tmp_path, &lockfile_path) {
                        writeln!(std::io::stderr(), "failed to rename lockfile: {e}").ok();
                        drop(fs::remove_file(&tmp_path));
                        return 1;
                    }
                }
                Err(e) => {
                    writeln!(std::io::stderr(), "failed to write lockfile: {e}").ok();
                    drop(fs::remove_file(&tmp_path));
                    return 1;
                }
            }
        }
    } else {
        write!(std::io::stdout(), "{lockfile_content}").ok();
    }

    0
}
