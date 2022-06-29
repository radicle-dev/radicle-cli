use std::ffi::OsString;
use std::{io::ErrorKind, iter, process};

use anyhow::anyhow;
use radicle_common::profile;

pub const NAME: &str = "rad";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Debug)]
enum Command {
    External(Vec<OsString>),
    Help,
    Version,
}

fn main() {
    match parse_args().map_err(Some).and_then(run) {
        Ok(_) => process::exit(0),
        Err(err) => {
            if let Some(err) = err {
                radicle_terminal::error(&format!("Error: rad: {}", err));
            }
            process::exit(1);
        }
    }
}

fn parse_args() -> anyhow::Result<Command> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut command = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("help") | Short('h') => {
                command = Some(Command::Help);
            }
            Long("version") => {
                command = Some(Command::Version);
            }
            Value(val) if command.is_none() => {
                if val == *"." {
                    command = Some(Command::External(vec![OsString::from("inspect")]));
                } else {
                    let args = iter::once(val)
                        .chain(iter::from_fn(|| parser.value().ok()))
                        .collect();

                    command = Some(Command::External(args))
                }
            }
            _ => return Err(anyhow::anyhow!(arg.unexpected())),
        }
    }

    Ok(command.unwrap_or_else(|| Command::External(vec![])))
}

fn print_version() {
    println!("{} {}", NAME, VERSION);
}

fn print_help() -> anyhow::Result<()> {
    print_version();
    println!("{}", DESCRIPTION);
    println!();

    rad_help::run(Default::default(), profile::default)
}

fn run(command: Command) -> Result<(), Option<anyhow::Error>> {
    match command {
        Command::Version => {
            print_version();
        }
        Command::Help => {
            print_help()?;
        }
        Command::External(args) => {
            let exe = args.first();

            match exe {
                Some(exe) => {
                    let exe = format!("{}-{}", NAME, exe.to_string_lossy());
                    let status = process::Command::new(exe.clone()).args(&args[1..]).status();

                    match status {
                        Ok(status) => {
                            if !status.success() {
                                return Err(None);
                            }
                        }
                        Err(err) => {
                            if let ErrorKind::NotFound = err.kind() {
                                return Err(Some(anyhow!("command `{}` not found", exe)));
                            } else {
                                return Err(Some(err.into()));
                            }
                        }
                    }
                }
                None => {
                    print_help()?;
                }
            }
        }
    }

    Ok(())
}
