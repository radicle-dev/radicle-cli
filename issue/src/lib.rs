#![allow(clippy::or_fun_call)]
use std::ffi::OsString;
use std::str::FromStr;

use anyhow::{anyhow, Context};

use radicle_common::args::{Args, Error, Help};

use radicle_common::{cobs, keys, person, profile, project};
use radicle_terminal as term;

use cobs::issue::*;
use cobs::Label;

pub const HELP: Help = Help {
    name: "issue",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad issue create [--title <title>] [--description <text>]
    rad issue remove <id>
    rad issue comment <id> [--description <text>]
    rad issue react <id> [--emoji <char>]
    rad issue list

Options

        --help      Print help
"#,
};

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct Metadata {
    title: String,
    labels: Vec<Label>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OperationName {
    Create,
    Comment,
    Reaction,
    Remove,
    List,
}

impl Default for OperationName {
    fn default() -> Self {
        Self::List
    }
}

#[derive(Debug)]
pub enum Operation {
    Create {
        title: Option<String>,
        description: Option<String>,
    },
    Remove {
        id: cobs::issue::IssueId,
    },
    Reaction {
        id: cobs::issue::IssueId,
        emoji: String,
    },
    Comment {
        id: cobs::issue::IssueId,
        description: Option<String>,
    },
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
        let mut op: Option<OperationName> = None;
        let mut id: Option<cobs::issue::IssueId> = None;
        let mut title: Option<String> = None;
        let mut emoji: Option<String> = None;
        let mut description: Option<String> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("title") if op == Some(OperationName::Create) => {
                    title = Some(parser.value()?.to_string_lossy().into());
                }
                Long("emoji") if op == Some(OperationName::Reaction) => {
                    emoji = Some(parser.value()?.to_string_lossy().into());
                }
                Long("description")
                    if op == Some(OperationName::Create) || op == Some(OperationName::Comment) =>
                {
                    description = Some(parser.value()?.to_string_lossy().into());
                }
                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "n" | "new" => op = Some(OperationName::Create),
                    "rm" | "remove" => op = Some(OperationName::Remove),
                    "l" | "list" => op = Some(OperationName::List),
                    "r" | "react" => op = Some(OperationName::Reaction),
                    "c" | "comment" => op = Some(OperationName::Comment),

                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                Value(val) if op.is_some() => {
                    let val = val
                        .to_str()
                        .ok_or_else(|| anyhow!("invalid operation name"))?;

                    id = Some(
                        IssueId::from_str(val)
                            .map_err(|_| anyhow!("invalid issue id '{}'", val))?,
                    );
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        let op = match op.unwrap_or_default() {
            OperationName::Create => Operation::Create { title, description },
            OperationName::Reaction => Operation::Reaction {
                id: id.ok_or_else(|| anyhow!("an issue id must be provided"))?,
                emoji: emoji.ok_or_else(|| anyhow!("a reaction emoji must be provided"))?,
            },
            OperationName::Remove => Operation::Remove {
                id: id.ok_or_else(|| anyhow!("an issue id to remove must be provided"))?,
            },
            OperationName::List => Operation::List,
            OperationName::Comment => Operation::Comment {
                id: id.ok_or_else(|| anyhow!("an issue id to comment must be provided"))?,
                description,
            },
        };

        Ok((Options { op }, vec![]))
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
        Operation::Create {
            title: Some(title),
            description: Some(description),
        } => {
            issues.create(&project, &title, &description, &[])?;
        }
        Operation::Reaction { id, emoji } => {
            if let Some(issue) = issues.get(&project, &id)? {
                let comment_id = term::comment_select(&issue).unwrap();
                issues.react(
                    &project,
                    &id,
                    comment_id,
                    cobs::shared::Reaction::new(char::from_str(&emoji)?)?,
                )?;
            }
        }
        Operation::Create { title, description } => {
            let meta = Metadata {
                title: title.unwrap_or("Enter a title".to_owned()),
                labels: vec![],
            };
            let yaml = serde_yaml::to_string(&meta)?;
            let doc = format!(
                "{}---\n\n{}",
                yaml,
                description.unwrap_or("Enter a description...".to_owned())
            );

            if let Some(text) = term::Editor::new().edit(&doc)? {
                let mut meta = String::new();
                let mut frontmatter = false;
                let mut lines = text.lines();

                while let Some(line) = lines.by_ref().next() {
                    if line.trim() == "---" {
                        if frontmatter {
                            break;
                        } else {
                            frontmatter = true;
                            continue;
                        }
                    }
                    if frontmatter {
                        meta.push_str(line);
                        meta.push('\n');
                    }
                }

                let description: String = lines.collect::<Vec<&str>>().join("\n");
                let meta: Metadata =
                    serde_yaml::from_str(&meta).context("failed to parse yaml front-matter")?;

                issues.create(&project, &meta.title, description.trim(), &meta.labels)?;
            }
        }
        Operation::List => {
            for (id, issue) in issues.all(&project)? {
                println!("{} {}", id, issue.title());
            }
        }
        Operation::Remove { id } => {
            issues.remove(&project, &id)?;
        }
        Operation::Comment { id, description } => {
            let doc = description.unwrap_or("Enter a description...".to_owned());
            if let Some(text) = term::Editor::new().edit(&doc)? {
                issues.comment(&project, &id, &text)?;
            }
        }
    }

    Ok(())
}
