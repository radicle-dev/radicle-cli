#![allow(clippy::or_fun_call)]
use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{
    cobs::{self, issue, patch, CommentId},
    keys, project,
};
use radicle_terminal as term;
use radicle_terminal::patch::Comment;

pub const HELP: Help = Help {
    name: "comment",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad comment <id> [-m <text>] [--reply-to <index>]

Options

    -m, --message               Comment message
        --reply-to <index>      Index of comment writing a reply for
        --help                  Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub id: cobs::Identifier,
    pub message: Comment,
    pub reply_index: Option<CommentId>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<cobs::Identifier> = None;
        let mut message = Comment::default();
        let mut reply_index: Option<CommentId> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("message") | Short('m') => {
                    let txt: String = parser.value()?.to_string_lossy().into();
                    message.append(&txt);
                }
                Long("reply-to") => {
                    let idx = parser
                        .value()?
                        .parse::<usize>()
                        .map_err(|_| anyhow!("index for `--reply-to` can't be parsed as usize"))?;

                    reply_index = Some(CommentId::from(idx));
                }
                Value(val) if id.is_none() => {
                    let val = val
                        .to_str()
                        .ok_or_else(|| anyhow!("object id specified is not UTF-8"))?;

                    id = Some(
                        cobs::Identifier::from_str(val)
                            .map_err(|_| anyhow!("invalid object id '{}'", val))?,
                    );
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((
            Options {
                id: id.ok_or_else(|| anyhow!("an object id must be provided"))?,
                message,
                reply_index,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let (project, _) = project::cwd()?;
    let cobs = cobs::store(&profile, &storage)?;
    let cob_id = options.id;

    let message = options.message.get("Enter a comment message...");
    if message.is_empty() {
        return Ok(());
    }

    if let Some(id) = cobs.resolve_id::<issue::Issue>(&project, &cob_id)? {
        if let Some(reply_to_index) = options.reply_index {
            cobs.issues()
                .reply(&project, &id, reply_to_index, &message)?;
        } else {
            cobs.issues().comment(&project, &id, &message)?;
        }
    } else if let Some((id, patch)) = cobs.resolve::<patch::Patch>(&project, &cob_id)? {
        if let Some(reply_to_index) = options.reply_index {
            cobs.patches()
                .reply(&project, &id, patch.version(), reply_to_index, &message)?;
        } else {
            cobs.patches()
                .comment(&project, &id, patch.version(), &message)?;
        }
    } else {
        anyhow::bail!("Couldn't find issue or patch {}", cob_id);
    }

    Ok(())
}
