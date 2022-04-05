use std::ffi::OsString;

pub use git2::{Oid, Reference};

pub use lnk_identities::refs;

use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "patch",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage
    rad patch [<option>...]

    Creates a new patch.
Options
    --list    Prints all patches (default: false)
    --help    Print help
"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub list: bool,
    pub verbose: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut list = false;
        let mut verbose = false;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("list") | Short('l') => {
                    list = true;
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { list, verbose }, vec![]))
    }
}

pub fn run(_options: Options) -> anyhow::Result<()> {
    term::warning("Not implemented yet!");

    Ok(())
}
