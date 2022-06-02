use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle_common as common;
use radicle_common::args::{Args, Error, Help};
use radicle_common::cobs::shared::CobIdentifier;
use radicle_common::{cobs, keys, person, profile, project};
use radicle_terminal as term;

use cobs::patch::RevisionId;

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

    -i, --interactive         Ask for confirmations
    -r, --revision <number>   Revision number to merge, defaults to the latest
        --help                Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub id: CobIdentifier,
    pub interactive: bool,
    pub revision: Option<RevisionId>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<CobIdentifier> = None;
        let mut revision: Option<RevisionId> = None;
        let mut interactive = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("interactive") | Short('i') => {
                    interactive = true;
                }
                Long("revision") | Short('r') => {
                    let value = parser.value()?;
                    let id =
                        RevisionId::from_str(value.to_str().unwrap_or_default()).map_err(|_| {
                            anyhow!("invalid revision number `{}`", value.to_string_lossy())
                        })?;
                    revision = Some(id);
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
                interactive,
                revision,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()
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
    let patch = patches
        .get(&urn, &id)?
        .ok_or_else(|| anyhow!("couldn't find patch {} locally", id))?;
    let head = repo.head()?;
    let branch = head.shorthand().unwrap_or("HEAD");
    let head_oid = head
        .target()
        .ok_or_else(|| anyhow!("cannot merge into detatched head; aborting"))?;
    let revision = options
        .revision
        .unwrap_or_else(|| patch.revisions.len() - 1);
    term::info!(
        "Merging revision {} of {} into {} ({})...",
        term::format::dim(format!("R{}", revision)),
        term::format::tertiary(common::fmt::cob(&id)),
        term::format::highlight(branch),
        term::format::secondary(common::fmt::oid(&head_oid))
    );

    if options.interactive && !term::confirm("Confirm?") {
        anyhow::bail!("merge aborted by user");
    }

    // TODO: Don't allow merging the same revision twice?

    patches.merge(&urn, &id, revision, head_oid.into())?;

    Ok(())
}
