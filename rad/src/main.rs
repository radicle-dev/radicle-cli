use std::{io::ErrorKind, process};

use rad_exe::cli::args::Command;
use rad_exe::cli::args::{self, Args};

use anyhow::anyhow;

fn main() {
    match parse_args().map_err(Some).and_then(|args| {
        let Args { global, command } = rad_exe::cli::args::sanitise_globals(args);
        run(global, command)
    }) {
        Ok(_) => process::exit(0),
        Err(err) => {
            if let Some(err) = err {
                rad_terminal::components::error(&format!("Error: rad: {}", err));
            }
            process::exit(1);
        }
    }
}

fn parse_args() -> anyhow::Result<Args> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut global = args::Global {
        rad_profile: None,
        rad_ssh_auth_sock: Default::default(),
        rad_quiet: false,
        rad_verbose: false,
    };
    let mut command = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("rad-profile") => {
                global.rad_profile = Some(parser.value()?.parse()?);
            }
            Long("rad-ssh-auth-sock") => {
                global.rad_ssh_auth_sock = parser.value()?.parse()?;
            }
            Long("rad-quiet") => {
                global.rad_quiet = true;
            }
            Long("rad-verbose") => {
                global.rad_verbose = true;
            }
            Value(val) if command.is_none() => {
                let cmd = val.to_string_lossy().into_owned();
                let mut args = vec![cmd];

                while let Some(a) = parser.next()? {
                    match a {
                        Long(s) => args.push(format!("--{}", s)),
                        Short(c) => args.push(format!("-{}", c)),
                        Value(v) => args.push(v.to_string_lossy().into_owned()),
                    }
                }
                command = Some(Command::External(args))
            }
            _ => return Err(anyhow::anyhow!(arg.unexpected())),
        }
    }

    Ok(Args {
        global,
        command: command.unwrap_or_else(|| Command::External(vec![])),
    })
}

fn run(_global: args::Global, command: Command) -> Result<(), Option<anyhow::Error>> {
    match command {
        Command::Identities(_) => unreachable!(),
        Command::Profile(_) => unreachable!(),

        Command::External(args) => {
            let exe = args.first();

            match exe {
                Some(exe) => {
                    let exe = format!("rad-{}", exe);
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
