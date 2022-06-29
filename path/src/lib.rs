use std::ffi::OsString;

use radicle_common::args::{Args, Error, Help};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "path",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad path [--help]
"#,
};

#[derive(Default, Eq, PartialEq)]
pub struct Options {}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options {}, vec![]))
    }
}

pub fn run(_options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;

    term::info!("{}", profile.paths().git_dir().display());

    Ok(())
}
