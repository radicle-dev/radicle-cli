use std::ffi::OsString;
use std::{io::ErrorKind, iter, process};

use anyhow::anyhow;

#[derive(Debug)]
enum Command {
    External(Vec<OsString>),
    Version,
}

fn main() {
    match parse_args().map_err(Some).and_then(run) {
        Ok(_) => process::exit(0),
        Err(err) => {
            if let Some(err) = err {
                rad_terminal::components::error(&format!("Error: rad: {}", err));
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
                command = Some(Command::External(vec![OsString::from("help")]));
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

fn run(command: Command) -> Result<(), Option<anyhow::Error>> {
    match command {
        Command::Version => {
            println!("rad {}", env!("CARGO_PKG_VERSION"));
        }
        Command::External(args) => {
            let exe = args.first();

            match exe {
                Some(exe) => {
                    let exe = format!("rad-{}", exe.to_string_lossy());
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
                    rad_help::run(Default::default())?;
                }
            }
        }
    }

    Ok(())
}
