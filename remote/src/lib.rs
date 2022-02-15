use std::ffi::OsString;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::PeerId;

use rad_common::{git, keys, profile, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "remote",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad remote add <name> <peer-id>
    rad remote ls

Examples

    rad remote add cloudhead hyn9diwfnytahjq8u3iw63h9jte1ydcatxax3saymwdxqu1zo645pe

Options

    --help   Print help
"#,
};

#[derive(Debug)]
pub enum Operation {
    Add { name: String, peer: PeerId },
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
        let mut peer: Option<PeerId> = None;
        let mut name: Option<String> = None;
        let mut op: Option<String> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if op.is_none() => {
                    op = Some(val.to_string_lossy().to_string());
                }
                Value(val) if name.is_none() => {
                    name = Some(val.to_string_lossy().to_string());
                }
                Value(val) if peer.is_none() => {
                    peer = Some(val.parse().context("invalid value specified for peer")?);
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        let op = match op {
            Some(op) => match op.as_str() {
                "add" => Operation::Add {
                    name: name.ok_or_else(|| anyhow!("a remote name must be specified"))?,
                    peer: peer.ok_or_else(|| anyhow!("a remote peer must be specified"))?,
                },
                "ls" => Operation::List,

                unknown => anyhow::bail!("unknown operation '{}'", unknown),
            },
            None => anyhow::bail!("an operation must be specified; see `rad remote --help`"),
        };

        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, _storage) = keys::storage(&profile, sock)?;
    let (urn, repo) = project::cwd()?;

    match options.op {
        Operation::Add { name, peer } => {
            let mut remote = git::remote(&urn, &peer, &name)?;
            remote.save(&repo)?;

            term::success!(
                "Remote {} successfully added",
                term::format::highlight(name)
            );
        }
        Operation::List => {
            todo!();
        }
    }

    Ok(())
}
