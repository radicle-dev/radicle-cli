#![allow(clippy::or_fun_call)]
use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{
    cobs::{self, issue::*, CobIdentifier, CommentId, Store as _},
    keys, person, profile, project,
};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "comment",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad comment <id> [--description <text>] [--reply-to <index>]

Options

    --description <text>    Comment text
    --reply-to <index>      Index of comment writing a reply for
    --help                  Print help
"#,
};

#[derive(Debug)]
pub struct Options {
    pub id: CobIdentifier,
    pub description: Option<String>,
    pub reply_index: Option<CommentId>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<CobIdentifier> = None;
        let mut description: Option<String> = None;
        let mut reply_index: Option<CommentId> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("description") => {
                    description = Some(parser.value()?.to_string_lossy().into());
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
                        CobIdentifier::from_str(val)
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
                description,
                reply_index,
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
    let issues = Issues::new(whoami.clone(), profile.paths(), &storage)?;
    let patches = cobs::patch::Patches::new(whoami, profile.paths(), &storage)?;
    let cob_id = options.id;

    let doc = options
        .description
        .unwrap_or("Enter a description...".to_owned());

    if let Some(text) = term::Editor::new().edit(&doc)? {
        if let Ok(id) = issues.resolve_id(&project, cob_id.clone()) {
            if let Some(reply_to_index) = options.reply_index {
                issues.reply(&project, &id, reply_to_index, &text)?;
            } else {
                issues.comment(&project, &id, &text)?;
            }
        } else if let Ok(id) = patches.resolve_id(&project, cob_id) {
            let patch = patches
                .get(&project, &id)?
                .ok_or_else(|| anyhow!("Couldn't get the patch"))?;
            if let Some(reply_to_index) = options.reply_index {
                patches.reply(&project, &id, patch.version(), reply_to_index, &text)?;
            } else {
                patches.comment(&project, &id, patch.version(), &text)?;
            }
        }
    }

    Ok(())
}
