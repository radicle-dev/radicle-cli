use std::ffi::OsString;
use std::str::FromStr;

use radicle_common::args::{Args, Error, Help};
use radicle_common::keys;
use radicle_terminal as term;

use librad::git::identities::{any, person, project, SomeIdentity};
use librad::git::Urn;

use link_identities::payload::{PersonPayload, ProjectPayload};

use anyhow::anyhow;

pub const HELP: Help = Help {
    name: "edit",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad edit [<urn>] [<option>...]

    Edits the identity document pointed to by the URN. If it isn't specified,
    the current project is edited.

Options

    --help              Print help
"#,
};

#[derive(Default, Debug, Eq, PartialEq)]
pub struct Options {
    pub urn: Option<Urn>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut urn: Option<Urn> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();

                    if let Ok(val) = Urn::from_str(&val) {
                        urn = Some(val);
                    } else {
                        return Err(anyhow!("invalid URN '{}'", val));
                    }
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { urn }, vec![]))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;

    let urn = options
        .urn
        .or_else(|| radicle_common::project::cwd().ok().map(|(urn, _)| urn))
        .ok_or_else(|| anyhow!("Couldn't get URN from either command line or cwd"))?;

    let identity = any::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("No project or person found for this URN"))?;

    match identity {
        SomeIdentity::Project(_) => {
            let payload = serde_json::to_string_pretty(
                project::verify(&storage, &urn)?
                    .ok_or_else(|| anyhow!("Couldn't get project's identity doc"))?
                    .payload(),
            )?;
            match term::Editor::new().edit(&payload)? {
                Some(updated_payload) => {
                    let payload: ProjectPayload = serde_json::from_str(&updated_payload)?;
                    project::update(&storage, &urn, None, payload, None)?;
                }
                None => return Err(anyhow!("Operation aborted!")),
            }
        }
        SomeIdentity::Person(_) => {
            let payload = serde_json::to_string_pretty(
                person::verify(&storage, &urn)?
                    .ok_or_else(|| anyhow!("Couldn't get person's identity doc"))?
                    .payload(),
            )?;
            match term::Editor::new().edit(&payload)? {
                Some(updated_payload) => {
                    let payload: PersonPayload = serde_json::from_str(&updated_payload)?;
                    person::update(&storage, &urn, None, payload, None)?;
                }
                None => return Err(anyhow!("Operation aborted!")),
            }
        }
        _ => {
            anyhow::bail!("Operation not supported for identity type of {}", urn)
        }
    };

    term::success!("Update successful!");

    Ok(())
}
