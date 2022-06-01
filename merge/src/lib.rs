use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};
use radicle_common::cobs::shared::CobIdentifier;
use radicle_common::{cobs, keys, person, profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "merge",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad merge [<id>] [<option>...]

    To specify a patch to merge, use the fully qualified patch id
    or an unambiguous prefix of it.

Options

    --help   Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub id: CobIdentifier,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<CobIdentifier> = None;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) => {
                    let val = val
                        .to_str()
                        .ok_or_else(|| anyhow!("patch id specified is not UTF-8"))?;

                    id = Some(
                        CobIdentifier::from_str(val)
                            .map_err(|_| anyhow!("invalid patch id '{}'", val))?,
                    );
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                id: id.ok_or_else(|| anyhow!("a patch id to merge must be provided"))?,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, _) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let profile = profile::default()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let _project = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("couldn't load project {} from local state", urn))?;
    let whoami = person::local(&storage)?;
    let patches = cobs::patch::Patches::new(whoami, profile.paths(), &storage)?;

    let id = match options.id {
        CobIdentifier::Full(id) => id,
        CobIdentifier::Prefix(prefix) => {
            let matches = patches.find(&urn, |p| p.to_string().starts_with(&prefix))?;

            match matches.as_slice() {
                [id] => *id,
                [_id, ..] => {
                    anyhow::bail!(
                        "patch id `{}` is ambiguous; please use the fully qualified id",
                        prefix
                    );
                }
                [] => {
                    anyhow::bail!("patch `{}` not found in local storage", prefix);
                }
            }
        }
    };
    let _patch = patches
        .get(&urn, &id)?
        .ok_or_else(|| anyhow!("couldn't find patch {} locally", id))?;

    term::info!("Merging {}...", id);

    Ok(())
}
