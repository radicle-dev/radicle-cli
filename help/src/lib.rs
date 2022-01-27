use rad_terminal::components as term;
use rad_terminal::components::{Args, Error, Help};

pub const NAME: &str = "help";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Radicle tools help";
pub const HELP: Help = Help {
    name: NAME,
    description: DESCRIPTION,
    version: VERSION,
    usage: "Usage: rad help [--help]",
};

const COMMANDS: &[(&str, &str)] = &[
    (rad_auth::NAME, rad_auth::DESCRIPTION),
    (rad_init::NAME, rad_init::DESCRIPTION),
    (rad_publish::NAME, rad_publish::DESCRIPTION),
    (rad_checkout::NAME, rad_checkout::DESCRIPTION),
    (rad_track::NAME, rad_track::DESCRIPTION),
    (rad_untrack::NAME, rad_untrack::DESCRIPTION),
    (rad_sync::NAME, rad_sync::DESCRIPTION),
    (crate::NAME, crate::DESCRIPTION),
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

    for (name, description) in COMMANDS {
        println!(
            "\t{} {}",
            term::format::bold(format!("{:-12}", name)),
            term::format::dim(description)
        );
    }
    println!();
    println!("See `rad <command> --help` to learn about a specific command.");
    println!();

    Ok(())
}
