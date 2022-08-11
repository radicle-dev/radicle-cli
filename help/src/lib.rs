use std::ffi::OsString;

use radicle_common::args::{Args, Error, Help};
use radicle_terminal as term;

#[cfg(feature = "ethereum")]
pub use rad_account;
pub use rad_auth;
pub use rad_checkout;
pub use rad_clone;
pub use rad_comment;
pub use rad_edit;
#[cfg(feature = "ethereum")]
pub use rad_ens;
#[cfg(feature = "ethereum")]
pub use rad_gov;
pub use rad_init;
pub use rad_inspect;
pub use rad_issue;
pub use rad_ls;
pub use rad_merge;
pub use rad_patch;
pub use rad_path;
pub use rad_pull;
pub use rad_push;
pub use rad_remote;
pub use rad_review;
pub use rad_rm;
pub use rad_self;
pub use rad_sync;
pub use rad_track;
pub use rad_untrack;

pub const HELP: Help = Help {
    name: "help",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: "Usage: rad help [--help]",
};

const COMMANDS: &[Help] = &[
    rad_auth::HELP,
    rad_init::HELP,
    rad_self::HELP,
    rad_inspect::HELP,
    rad_clone::HELP,
    rad_ls::HELP,
    rad_remote::HELP,
    rad_push::HELP,
    rad_pull::HELP,
    rad_checkout::HELP,
    rad_track::HELP,
    rad_untrack::HELP,
    rad_sync::HELP,
    #[cfg(feature = "ethereum")]
    rad_ens::HELP,
    #[cfg(feature = "ethereum")]
    rad_account::HELP,
    rad_rm::HELP,
    rad_edit::HELP,
    crate::HELP,
];

#[derive(Default)]
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
    println!("Usage: rad <command> [--help]");

    if ctx.profile().is_err() {
        println!();
        println!(
            "{}",
            term::format::highlight("It looks like this is your first time using radicle.")
        );
        println!(
            "{}",
            term::format::highlight("To get started, use `rad auth` to authenticate.")
        );
        println!();
    }

    println!("Common `rad` commands used in various situations:");
    println!();

    for help in COMMANDS {
        println!(
            "\t{} {}",
            term::format::bold(format!("{:-12}", help.name)),
            term::format::dim(help.description)
        );
    }
    println!();
    println!("See `rad <command> --help` to learn about a specific command.");
    println!();

    Ok(())
}
