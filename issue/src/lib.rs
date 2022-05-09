use std::ffi::OsString;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};

use radicle_common::{cobs, keys, person, profile, project};
use radicle_terminal as term;

use cobs::issue::*;

pub const HELP: Help = Help {
    name: "issue",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad issue create <name> <peer-id> [-f | --fetch]
    rad issue delete <name | peer-id>
    rad issue list

Options

        --help      Print help
"#,
};

#[derive(Debug)]
pub enum Operation {
    Create { title: String, description: String },
    Delete {},
    List,
}

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub op: Operation,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<Operation> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("title") => {
                    if let Some(Operation::Create { ref mut title, .. }) = op {
                        *title = parser.value()?.to_string_lossy().into();
                    } else {
                        return Err(anyhow!(arg.unexpected()));
                    }
                }
                Long("description") => {
                    if let Some(Operation::Create {
                        ref mut description,
                        ..
                    }) = op
                    {
                        *description = parser.value()?.to_string_lossy().into();
                    } else {
                        return Err(anyhow!(arg.unexpected()));
                    }
                }
                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "c" | "create" => {
                        op = Some(Operation::Create {
                            title: String::new(),
                            description: String::new(),
                        })
                    }
                    "r" | "delete" => op = Some(Operation::Delete {}),
                    "l" | "list" => op = Some(Operation::List),

                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((
            Options {
                op: op.unwrap_or(Operation::List),
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let (project, _) = project::cwd()?;
    let whoami = person::local(&storage)?;
    let issues = Issues::new(whoami, profile.paths(), &storage)?;

    match options.op {
        Operation::Create { title, description } => {
            issues.create(&project, &title, &description)?;
        }
        Operation::List => {
            for (id, issue) in issues.all(&project)? {
                println!("{} {}", id, issue.title());
            }
        }
        Operation::Delete { .. } => {
            todo!();
        }
    }

    Ok(())
}
