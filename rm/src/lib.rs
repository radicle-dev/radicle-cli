use std::ffi::OsString;
use std::fs;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::Urn;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{keys, profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "rm",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad rm <urn> [<option>...]

Options

    --help    Print help
"#,
};

pub struct Options {
    urn: Urn,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut urn: Option<Urn> = None;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                urn: urn.ok_or_else(|| {
                    anyhow!("a URN to remove must be provided; see `rad rm --help`")
                })?,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;

    if project::get(&storage, &options.urn)?.is_none() {
        anyhow::bail!("project {} does not exist", options.urn);
    }
    term::warning("Warning: experimental tool; use at your own risk!");

    rad_untrack::execute(&options.urn, rad_untrack::Options { peer: None })?;

    let monorepo = profile.paths().git_dir();
    let namespace = monorepo
        .join("refs")
        .join("namespaces")
        .join(options.urn.encode_id());

    if term::confirm(format!(
        "Are you sure you would like to delete {}?",
        term::format::dim(namespace.display())
    )) {
        fs::remove_dir_all(namespace)?;
        term::success!("Successfully removed project {}", options.urn);
    }

    Ok(())
}
