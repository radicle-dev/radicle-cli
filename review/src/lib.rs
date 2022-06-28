use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use common::cobs::patch::Verdict;
use radicle_common as common;
use radicle_common::args::{Args, Error, Help};
use radicle_common::cobs::patch::Patch;
use radicle_common::{cobs, keys, profile, project};
use radicle_terminal as term;
use radicle_terminal::patch::Comment;

use cobs::patch::RevisionIx;

pub const HELP: Help = Help {
    name: "review",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad review [<id>] [--accept|--reject] [-m [<string>]] [<option>...]

    To specify a patch to review, use the fully qualified patch id
    or an unambiguous prefix of it.

Options

    -r, --revision <number>   Revision number to review, defaults to the latest
    -m, --message [<string>]  Provide a comment with the review
        --no-wmessage         Don't provide a comment with the review
        --help                Print help
"#,
};

/// Review help message.
pub const REVIEW_HELP_MSG: &str = r#"
<!--
You may enter a review comment here. If you leave this blank,
no comment will be attached to your review.

Markdown supported.
-->
"#;

#[derive(Debug)]
pub struct Options {
    pub id: cobs::Identifier,
    pub revision: Option<RevisionIx>,
    pub message: Comment,
    pub verdict: Option<Verdict>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<cobs::Identifier> = None;
        let mut revision: Option<RevisionIx> = None;
        let mut message = Comment::default();
        let mut verdict = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("revision") | Short('r') => {
                    let value = parser.value()?;
                    let id =
                        RevisionIx::from_str(value.to_str().unwrap_or_default()).map_err(|_| {
                            anyhow!("invalid revision number `{}`", value.to_string_lossy())
                        })?;
                    revision = Some(id);
                }
                Long("message") | Short('m') => {
                    message = Comment::Text(parser.value()?.to_string_lossy().into());
                }
                Long("no-message") => {
                    message = Comment::Blank;
                }
                Long("accept") if verdict.is_none() => {
                    verdict = Some(Verdict::Accept);
                }
                Long("reject") if verdict.is_none() => {
                    verdict = Some(Verdict::Reject);
                }
                Value(val) => {
                    let val = val
                        .to_str()
                        .ok_or_else(|| anyhow!("patch id specified is not UTF-8"))?;

                    id = Some(
                        cobs::Identifier::from_str(val)
                            .map_err(|_| anyhow!("invalid patch id '{}'", val))?,
                    );
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                id: id.ok_or_else(|| anyhow!("a patch id to review must be provided"))?,
                message,
                revision,
                verdict,
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
    let cobs = cobs::store(&profile, &storage)?;
    let patches = cobs.patches();

    let (patch_id, mut patch) = patches
        .resolve::<Patch>(&urn, &options.id)?
        .ok_or_else(|| anyhow!("couldn't find patch {} locally", options.id))?;
    let patch_id_pretty = term::format::tertiary(common::fmt::cob(&patch_id));
    let revision_ix = options.revision.unwrap_or_else(|| patch.version());
    let _revision = patch
        .revisions
        .get(revision_ix)
        .ok_or_else(|| anyhow!("revision R{} does not exist", revision_ix))?;
    let message = options.message.get(REVIEW_HELP_MSG);

    patch.author.resolve(&storage).ok();

    let verdict_pretty = match options.verdict {
        Some(Verdict::Accept) => term::format::highlight("Accept"),
        Some(Verdict::Reject) => term::format::negative("Reject"),
        None => term::format::dim("Review"),
    };
    if !term::confirm(format!(
        "{} {} {} by {}?",
        verdict_pretty,
        patch_id_pretty,
        term::format::dim(format!("R{}", revision_ix)),
        term::format::tertiary(patch.author.name())
    )) {
        anyhow::bail!("Patch review aborted");
    }

    patches.review(
        &urn,
        &patch_id,
        revision_ix,
        options.verdict,
        message,
        vec![],
    )?;

    match options.verdict {
        Some(Verdict::Accept) => {
            term::success!(
                "Patch {} {}",
                patch_id_pretty,
                term::format::highlight("accepted")
            );
        }
        Some(Verdict::Reject) => {
            term::success!(
                "Patch {} {}",
                patch_id_pretty,
                term::format::negative("rejected")
            );
        }
        None => {
            term::success!("Patch {} reviewed", patch_id_pretty);
        }
    }

    Ok(())
}
