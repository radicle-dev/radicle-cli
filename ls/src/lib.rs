use std::ffi::OsString;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "ls",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad ls [<option>...]

Options

    --help    Print help
"#,
};

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
    let storage = profile::read_only(&profile)?;
    let projs = project::list(&storage)?;
    let mut table = term::Table::default();

    for (urn, meta, head) in projs {
        let head = head
            .map(|h| format!("{:.7}", h.to_string()))
            .unwrap_or_else(String::new);

        table.push([
            term::format::bold(meta.name),
            term::format::tertiary(urn),
            term::format::secondary(head),
            term::format::italic(meta.description),
        ]);
    }
    table.render();

    Ok(())
}
