use rad_terminal::components as term;
use rad_terminal::components::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "help",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: "Usage: rad help [--help]",
};

const COMMANDS: &[Help] = &[
    rad_auth::HELP,
    rad_init::HELP,
    rad_show::HELP,
    rad_account::HELP,
    rad_ls::HELP,
    rad_push::HELP,
    rad_checkout::HELP,
    rad_track::HELP,
    rad_untrack::HELP,
    rad_sync::HELP,
    rad_ens::HELP,
    crate::HELP,
];

#[derive(Default)]
pub struct Options {}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }
        Ok(Options {})
    }
}

pub fn run(_options: Options) -> anyhow::Result<()> {
    println!("Usage: rad <command> [--help]");

    if rad_common::profile::default().is_err() {
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
