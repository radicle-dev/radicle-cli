#![allow(clippy::or_fun_call)]
use std::convert::TryFrom;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{git, profile};
use radicle_terminal as term;

use librad::git::identities::any;
use librad::git::storage::ReadOnlyStorage;
use librad::git::types::Reference;
use librad::git::Urn;

use anyhow::anyhow;

use chrono::prelude::*;
use json_color::{Color, Colorizer};

pub const HELP: Help = Help {
    name: "inspect",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad inspect <path> [<option>...]
    rad inspect <urn> [<option>...]
    rad inspect

    Inspects the given path or URN. If neither is specified,
    the current project is inspected.

Options

    --id        Return the ID without the URN scheme
    --payload   Inspect the object's payload
    --refs      Inspect the object's refs on the local device (requires `tree`)
    --history   Show object's history
    --help      Print help
"#,
};

#[derive(Default, Debug, Eq, PartialEq)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub urn: Option<Urn>,
    pub refs: bool,
    pub payload: bool,
    pub history: bool,
    pub id: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut path: Option<PathBuf> = None;
        let mut urn: Option<Urn> = None;
        let mut refs = false;
        let mut payload = false;
        let mut history = false;
        let mut id = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("refs") => {
                    refs = true;
                }
                Long("payload") => {
                    payload = true;
                }
                Long("history") => {
                    history = true;
                }
                Long("id") => {
                    id = true;
                }
                Value(val) if path.is_none() && urn.is_none() => {
                    let val = val.to_string_lossy();

                    if let Ok(val) = Urn::from_str(&val) {
                        urn = Some(val);
                    } else if val.starts_with("rad:git:") {
                        return Err(anyhow!("invalid URN '{}'", val));
                    } else if let Ok(val) = PathBuf::from_str(&val) {
                        path = Some(val);
                    } else {
                        return Err(anyhow!("invalid path or URN '{}'", val));
                    }
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                id,
                path,
                payload,
                history,
                refs,
                urn,
            },
            vec![],
        ))
    }
}

// Used for JSON Colorizing for now
fn colorizer() -> Colorizer {
    Colorizer::new()
        .null(Color::Cyan)
        .boolean(Color::Yellow)
        .number(Color::Magenta)
        .string(Color::Green)
        .key(Color::Blue)
        .build()
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let storage = profile::read_only(&profile)?;

    let urn = if let Some(urn) = options.urn {
        urn
    } else {
        let repo =
            git::Repository::open(options.path.unwrap_or_else(|| Path::new(".").to_path_buf()))?;

        git::rad_remote(&repo)?.url.urn
    };

    let colorizer = colorizer();

    if options.refs {
        let path = profile.paths().git_dir().join("refs").join("namespaces");

        Command::new("tree")
            .current_dir(path)
            .args([&urn.encode_id(), "--noreport", "--prune"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait()?;
    } else if options.payload {
        let payload = any::get(&storage, &urn)
            .map(|o| o.map(|p| p.payload()))
            .map_err(|_| anyhow::anyhow!("Couldn't load project or person."))?
            .ok_or(anyhow::anyhow!("No project or person found for this URN"))?;

        println!(
            "{}",
            colorizer.colorize_json_str(&serde_json::to_string_pretty(&payload)?)?
        );
    } else if options.history {
        let branch = Reference::try_from(&urn)?;
        match storage.reference(&branch) {
            Ok(Some(reference)) => {
                let mut tip = reference.peel_to_commit()?;

                for i in 0.. {
                    let tree = tip.tree()?;
                    let entry = tree
                        .get(0)
                        .ok_or(anyhow!("Couldn't get the first tree entry"))?
                        .id();
                    let blob = storage
                        .find_object(Box::new(entry))?
                        .ok_or(anyhow!(
                            "Couldn't find the object being pointed to by first tree entry"
                        ))?
                        .into_blob()
                        .map_err(|_| anyhow!("First tree entry is not a blob"))?;
                    let content: serde_json::Value = serde_json::from_slice(blob.content())?;
                    let timezone = if tip.time().sign() == '+' {
                        FixedOffset::east(tip.time().offset_minutes() * 60)
                    } else {
                        FixedOffset::west(tip.time().offset_minutes() * 60)
                    };
                    let time = DateTime::<Utc>::from(
                        std::time::UNIX_EPOCH
                            + std::time::Duration::from_secs(tip.time().seconds() as u64),
                    )
                    .with_timezone(&timezone)
                    .to_rfc2822();

                    print!(
                        "{}",
                        term::TextBox::new(format!(
                            "{}\ncommit {}\nblob   {}\ndate   {}\n\n{}",
                            term::format::yellow(format!("tree   {}", tree.id())),
                            term::format::dim(tip.id()),
                            term::format::dim(blob.id()),
                            term::format::dim(time),
                            colorizer
                                .colorize_json_str(&serde_json::to_string_pretty(&content)?)?,
                        ))
                        .first(i == 0)
                        .last(false)
                    );

                    match tip.parent(0) {
                        Ok(p) => tip = p,
                        Err(_) => break,
                    }
                }

                println!(" └─ {}", term::format::highlight(urn.to_string()));
                println!();
            }

            _ => return Err(anyhow!("Couldn't find reference to {} in storage", urn)),
        }
    } else if options.id {
        term::info!("{}", term::format::highlight(urn.encode_id()));
    } else {
        term::info!("{}", term::format::highlight(urn));
    }

    Ok(())
}
