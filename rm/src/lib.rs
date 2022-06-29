use std::ffi::OsString;
use std::fs;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::Urn;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "rm",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad rm <urn> [<option>...]

Options

    -i        Prompt before removal
    --help    Print help
"#,
};

pub struct Options {
    urn: Urn,
    prompt: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut urn: Option<Urn> = None;
        let mut prompt = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Short('i') => {
                    prompt = true;
                }
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
                prompt,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let storage = profile::read_only(&profile)?;

    if project::get(&storage, &options.urn)?.is_none() {
        anyhow::bail!("project {} does not exist", options.urn);
    }
    term::warning("Experimental tool; use at your own risk!");

    let monorepo = profile.paths().git_dir();
    let namespace = monorepo
        .join("refs")
        .join("namespaces")
        .join(options.urn.encode_id());

    if !options.prompt
        || term::confirm(format!(
            "Are you sure you would like to delete {}?",
            term::format::dim(namespace.display())
        ))
    {
        rad_untrack::execute(&options.urn, rad_untrack::Options { peer: None }, &profile)?;
        fs::remove_dir_all(namespace)?;
        term::success!("Successfully removed project {}", options.urn);
    }

    Ok(())
}
