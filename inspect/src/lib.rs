use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rad_common::{git, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

use anyhow::Context;

pub const HELP: Help = Help {
    name: "inspect",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad inspect [<path>] [<option>...]

    Inspects the given path, or current working directory.

Options

    --help      Print help
"#,
};

#[derive(Default, Eq, PartialEq)]
pub struct Options {
    pub path: Option<PathBuf>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut path: Option<PathBuf> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if path.is_none() => {
                    let val = val.to_string_lossy();
                    let val = PathBuf::from_str(&val).context(format!("invalid path '{}'", val))?;

                    path = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { path }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let repo = git::Repository::open(options.path.unwrap_or_else(|| Path::new(".").to_path_buf()))?;
    let urn = project::remote(&repo)?.url.urn;

    term::info!("{}", term::format::highlight(urn));

    Ok(())
}
