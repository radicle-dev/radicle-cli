use std::ffi::OsString;
use std::fmt::Write;
use std::str::FromStr;

use anyhow::{anyhow, Context};

use radicle_common as common;
use radicle_common::args::{Args, Error, Help};
use radicle_common::cobs::patch::{Patch, PatchId};
use radicle_common::patch::MergeStyle;
use radicle_common::{cobs, git, keys, project};
use radicle_terminal as term;

use cobs::patch::RevisionIx;

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

/// Merge commit help message.
const MERGE_HELP_MSG: &[&str] = &[
    "# Check the commit message for this merge and make sure everything looks good,",
    "# or make any necessary change.",
    "#",
    "# Lines starting with '#' will be ignored, and an empty message aborts the commit.",
    "#",
    "# vim: ft=gitcommit",
];

#[derive(Debug)]
pub struct Options {
    pub id: cobs::Identifier,
    pub interactive: bool,
    pub revision: Option<RevisionIx>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut id: Option<cobs::Identifier> = None;
        let mut revision: Option<RevisionIx> = None;
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
                        RevisionIx::from_str(value.to_str().unwrap_or_default()).map_err(|_| {
                            anyhow!("invalid revision number `{}`", value.to_string_lossy())
                        })?;
                    revision = Some(id);
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
                id: id.ok_or_else(|| anyhow!("a patch id to merge must be provided"))?,
                interactive,
                revision,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    //
    // Setup
    //
    let (urn, repo) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let _project = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("couldn't load project {} from local state", urn))?;
    let cobs = cobs::store(&profile, &storage)?;
    let patches = cobs.patches();

    if repo.head_detached()? {
        anyhow::bail!("HEAD is in a detached state; can't merge");
    }

    //
    // Get patch information
    //
    let (patch_id, mut patch) = patches
        .resolve::<Patch>(&urn, &options.id)?
        .ok_or_else(|| anyhow!("couldn't find patch {} locally", &options.id))?;
    patch.author.resolve(&storage).ok();

    let head = repo.head()?;
    let branch = head
        .shorthand()
        .ok_or_else(|| anyhow!("invalid head branch"))?;
    let head_oid = head
        .target()
        .ok_or_else(|| anyhow!("cannot merge into detatched head; aborting"))?;
    let revision_id = options.revision.unwrap_or_else(|| patch.version());
    let revision = patch
        .revisions
        .get(revision_id)
        .ok_or_else(|| anyhow!("revision R{} does not exist", revision_id))?;

    //
    // Analyze merge
    //
    let patch_commit = repo
        .find_annotated_commit(revision.oid.into())
        .context("patch head not found in local repository")?;
    let (merge, _merge_pref) = repo.merge_analysis(&[&patch_commit])?;

    let merge_style = if merge.is_fast_forward() {
        // The given merge input is a fast-forward from HEAD and no merge needs to be performed.
        // Instead, the client can apply the input commits to its HEAD.
        MergeStyle::FastForward
    } else if merge.is_normal() {
        // A “normal” merge; both HEAD and the given merge input have diverged from their common
        // ancestor. The divergent commits must be merged.
        //
        // Let's check if there are potential merge conflicts.
        let our_commit = head.peel_to_commit()?;
        let their_commit = repo.find_commit(revision.oid.into())?;

        let index = repo
            .merge_commits(&our_commit, &their_commit, None)
            .context("failed to perform merge analysis")?;

        if index.has_conflicts() {
            return Err(common::Error::WithHint {
                err: anyhow!("patch conflicts with {}", branch),
                hint: "Patch must be rebased before it can be merged.",
            }
            .into());
        }
        MergeStyle::Commit
    } else if merge.is_up_to_date() {
        term::info!(
            "✓ Patch {} is already part of {}",
            term::format::tertiary(patch_id),
            term::format::highlight(branch)
        );

        return Ok(());
    } else if merge.is_unborn() {
        anyhow::bail!("HEAD does not point to a valid commit");
    } else {
        anyhow::bail!(
            "no merge is possible between {} and {}",
            head_oid,
            revision.oid
        );
    };

    let merge_style_pretty = match merge_style {
        MergeStyle::FastForward => term::format::style(merge_style.to_string())
            .dim()
            .italic()
            .to_string(),
        MergeStyle::Commit => term::format::style(merge_style.to_string())
            .yellow()
            .italic()
            .to_string(),
    };

    term::info!(
        "{} {} {} ({}) by {} into {} ({}) via {}...",
        term::format::bold("Merging"),
        term::format::tertiary(common::fmt::cob(&patch_id)),
        term::format::dim(format!("R{}", revision_id)),
        term::format::secondary(common::fmt::oid(&revision.oid)),
        term::format::tertiary(patch.author.name()),
        term::format::highlight(branch),
        term::format::secondary(common::fmt::oid(&head_oid)),
        merge_style_pretty
    );

    if options.interactive && !term::confirm("Confirm?") {
        anyhow::bail!("merge aborted by user");
    }

    //
    // Perform merge
    //
    match merge_style {
        MergeStyle::Commit => {
            merge_commit(&repo, patch_id, &patch_commit, &patch, cobs.whoami.urn())?;
        }
        MergeStyle::FastForward => {
            fast_forward(&repo, &revision.oid)?;
        }
    }

    term::success!(
        "Updated {} {} -> {} via {}",
        term::format::highlight(branch),
        term::format::secondary(common::fmt::oid(&head_oid)),
        term::format::secondary(common::fmt::oid(&revision.oid)),
        merge_style_pretty
    );

    //
    // Update patch COB
    //
    // TODO: Don't allow merging the same revision twice?
    patches.merge(&urn, &patch_id, revision_id, head_oid.into())?;

    term::success!(
        "Patch state updated, use {} to publish",
        term::format::secondary("`rad push`")
    );

    Ok(())
}

// Perform git merge.
//
// This does not touch the COB state.
//
// Nb. Merge can fail even though conflicts were not detected if there are some
// files in the repo that are not checked in. This is because the merge conflict
// simulation only operates on the commits, not the checkout.
fn merge_commit(
    repo: &git::Repository,
    patch_id: PatchId,
    patch_commit: &git::AnnotatedCommit,
    patch: &Patch,
    whoami: common::Urn,
) -> anyhow::Result<()> {
    let description = patch.description().trim();
    let mut merge_opts = git::MergeOptions::new();
    let mut merge_msg = format!(
        "Merge patch '{}' from {}",
        common::fmt::cob(&patch_id),
        patch.author.name()
    );
    write!(&mut merge_msg, "\n\n")?;

    if !description.is_empty() {
        write!(&mut merge_msg, "{}", patch.description().trim())?;
        write!(&mut merge_msg, "\n\n")?;
    }
    writeln!(&mut merge_msg, "Rad-Patch: {}", patch_id)?;
    writeln!(&mut merge_msg, "Rad-Author: {}", patch.author.urn())?;
    writeln!(
        &mut merge_msg,
        "Rad-Peer: {}",
        patch.author.peer.default_encoding()
    )?;
    writeln!(&mut merge_msg, "Rad-Committer: {}", whoami)?;
    writeln!(&mut merge_msg)?;
    writeln!(&mut merge_msg, "{}", MERGE_HELP_MSG.join("\n").as_str())?;

    // Offer user the chance to edit the message before committing.
    let merge_msg = match term::Editor::new()
        .require_save(true)
        .trim_newlines(true)
        .extension(".git-commit")
        .edit(&merge_msg)
        .unwrap()
    {
        Some(s) => s
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n"),
        None => anyhow::bail!("user aborted merge"),
    };

    // Empty message aborts merge.
    if merge_msg.trim().is_empty() {
        anyhow::bail!("user aborted merge");
    }

    // Perform merge (nb. this does not commit).
    repo.merge(&[patch_commit], Some(merge_opts.patience(true)), None)
        .context("merge failed")?;

    // Commit staged changes.
    let commit = repo.find_commit(patch_commit.id())?;
    let author = commit.author();
    let committer = repo
        .signature()
        .context("git user name or email not configured")?;

    let tree = repo.index()?.write_tree()?;
    let tree = repo.find_tree(tree)?;
    let parents = &[&repo.head()?.peel_to_commit()?, &commit];

    repo.commit(
        Some("HEAD"),
        &author,
        &committer,
        &merge_msg,
        &tree,
        parents,
    )
    .context("merge commit failed")?;

    // Cleanup merge state.
    repo.cleanup_state().context("merge state cleanup failed")?;

    Ok(())
}

/// Perform fast-forward merge of patch.
fn fast_forward(repo: &git::Repository, patch_oid: &git::Oid) -> anyhow::Result<()> {
    let oid = patch_oid.to_string();
    let args = ["merge", "--ff-only", &oid];

    term::subcommand(format!("git {}", args.join(" ")));
    let output = git::git(
        repo.workdir()
            .ok_or_else(|| anyhow!("cannot fast-forward in bare repo"))?,
        args,
    )
    .context("fast-forward failed")?;

    term::blob(output);

    Ok(())
}
