#![allow(clippy::or_fun_call)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::for_kv_map)]
use std::convert::TryFrom;
use std::ffi::OsString;
use std::path::Path;
use std::str::FromStr;

use anyhow::anyhow;

use common::cobs::patch::Verdict;
use librad::git::identities::local::LocalIdentity;
use librad::git::storage::ReadOnlyStorage;
use librad::git::Storage;
use librad::git_ext::{Oid, RefLike};
use librad::profile::Profile;

use radicle_common as common;
use radicle_common::args::{Args, Error, Help};
use radicle_common::cobs::patch::{MergeTarget, Patch, PatchId, PatchStore};
use radicle_common::tokio;
use radicle_common::{cobs, git, keys, patch, project, sync};
use radicle_terminal as term;
use radicle_terminal::patch::Comment;

pub const HELP: Help = Help {
    name: "patch",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad patch [<option>...]

Create options

    -u, --update [<id>]        Update an existing patch (default: no)
        --[no-]sync            Sync patch to seed (default: sync)
        --[no-]push            Push patch head to storage (default: true)
    -m, --message [<string>]   Provide a comment message to the patch or revision (default: prompt)
        --no-message           Leave the patch or revision comment message blank

Options

    -l, --list                 List all patches (default: false)
        --help                 Print help
"#,
};

pub const PATCH_MSG: &str = r#"
<!--
Please enter a patch message for your changes. An empty
message aborts the patch proposal.

The first line is the patch title. The patch description
follows, and must be separated with a blank line, just
like a commit message. Markdown is supported in the title
and description.
-->
"#;

pub const REVISION_MSG: &str = r#"
<!--
Please enter a comment message for your patch update. Leaving this
blank is also okay.
-->
"#;

#[derive(Debug)]
pub enum Update {
    No,
    Any,
    Patch(cobs::Identifier),
}

impl Default for Update {
    fn default() -> Self {
        Self::No
    }
}

#[derive(Default, Debug)]
pub struct Options {
    pub list: bool,
    pub verbose: bool,
    pub sync: bool,
    pub push: bool,
    pub update: Update,
    pub message: Comment,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut list = false;
        let mut verbose = false;
        let mut sync = true;
        let mut message = Comment::default();
        let mut push = true;
        let mut update = Update::default();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("list") | Short('l') => {
                    list = true;
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("message") | Short('m') => {
                    let txt: String = parser.value()?.to_string_lossy().into();
                    message.append(&txt);
                }
                Long("no-message") => {
                    message = Comment::Blank;
                }
                Long("update") | Short('u') => {
                    if let Ok(val) = parser.value() {
                        let val = val
                            .to_str()
                            .ok_or_else(|| anyhow!("patch id specified is not UTF-8"))?;
                        let id = cobs::Identifier::from_str(val)
                            .map_err(|_| anyhow!("invalid patch id '{}'", val))?;

                        update = Update::Patch(id);
                    } else {
                        update = Update::Any;
                    }
                }
                Long("sync") => {
                    sync = true;
                }
                Long("no-sync") => {
                    sync = false;
                }
                Long("push") => {
                    push = true;
                }
                Long("no-push") => {
                    push = false;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                list,
                sync,
                message,
                push,
                update,
                verbose,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let project = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("couldn't load project {} from local state", urn))?;

    if options.list {
        list(&storage, Some(repo), &profile, &project, options)?;
    } else {
        create(&storage, &profile, &project, &repo, options)?;
    }

    Ok(())
}

fn list(
    storage: &Storage,
    repo: Option<git::Repository>,
    profile: &Profile,
    project: &project::Metadata,
    options: Options,
) -> anyhow::Result<()> {
    if options.sync {
        let rt = tokio::runtime::Runtime::new()?;

        term::sync::sync(
            project.urn.clone(),
            sync::seeds(profile)?,
            sync::Mode::Fetch,
            profile,
            term::signer(profile)?,
            &rt,
        )?;
    }

    let cobs = cobs::store(profile, storage)?;
    let patches = cobs.patches();
    let proposed = patches.proposed(&project.urn)?;
    let monorepo = git::Repository::open_bare(profile.paths().git_dir())?;

    // Patches the user authored.
    let mut own = Vec::new();
    // Patches other users authored.
    let mut other = Vec::new();

    for (id, patch) in proposed {
        if *patch.author.urn() == cobs.whoami.urn() {
            own.push((id, patch));
        } else {
            other.push((id, patch));
        }
    }
    term::blank();
    term::print(&term::format::badge_positive("YOU PROPOSED"));

    if own.is_empty() {
        term::blank();
        term::print(&term::format::italic("Nothing to show."));
    } else {
        for (id, patch) in &mut own {
            term::blank();

            print(&cobs.whoami, id, patch, project, &monorepo, &repo, storage)?;
        }
    }
    term::blank();
    term::print(&term::format::badge_secondary("OTHERS PROPOSED"));

    if other.is_empty() {
        term::blank();
        term::print(&term::format::italic("Nothing to show."));
    } else {
        for (id, patch) in &mut other {
            term::blank();

            print(&cobs.whoami, id, patch, project, &monorepo, &repo, storage)?;
        }
    }
    term::blank();

    Ok(())
}

fn update(
    patch: Patch,
    patch_id: PatchId,
    base: &git::Oid,
    head: &git::Oid,
    patches: &PatchStore,
    project: &project::Metadata,
    repo: &git::Repository,
    options: Options,
    profile: &Profile,
) -> anyhow::Result<()> {
    let (current, current_revision) = patch.latest();

    if &*current_revision.oid == head {
        term::info!("Nothing to do, patch is already up to date.");
        return Ok(());
    }

    term::info!(
        "{} {} ({}) -> {} ({})",
        term::format::tertiary(common::fmt::cob(&patch_id)),
        term::format::dim(format!("R{}", current)),
        term::format::secondary(common::fmt::oid(&current_revision.oid)),
        term::format::dim(format!("R{}", current + 1)),
        term::format::secondary(common::fmt::oid(head)),
    );
    let message = options.message.get(REVISION_MSG);

    // Difference between the two revisions.
    term::patch::print_commits_ahead_behind(repo, *head, *current_revision.oid)?;
    term::blank();

    if !term::confirm("Continue?") {
        anyhow::bail!("patch update aborted by user");
    }

    let new = patches.update(&project.urn, &patch_id, message, *base, *head)?;
    assert_eq!(new, current + 1);

    term::blank();
    term::success!("Patch {} updated 🌱", term::format::highlight(patch_id));
    term::blank();

    if options.sync {
        let rt = tokio::runtime::Runtime::new()?;

        term::sync::sync(
            project.urn.clone(),
            sync::seeds(profile)?,
            sync::Mode::Push,
            profile,
            term::signer(profile)?,
            &rt,
        )?;
    }

    Ok(())
}

fn create(
    storage: &Storage,
    profile: &Profile,
    project: &project::Metadata,
    repo: &git::Repository,
    options: Options,
) -> anyhow::Result<()> {
    term::headline(&format!(
        "🌱 Creating patch for {}",
        term::format::highlight(&project.name)
    ));
    let cobs = cobs::store(profile, storage)?;
    let patches = cobs.patches();

    // `HEAD`; This is what we are proposing as a patch.
    let head = repo.head()?;
    let head_oid = head.target().ok_or(anyhow!("invalid HEAD ref; aborting"))?;
    let head_commit = repo.find_commit(head_oid)?;
    let head_branch = head
        .shorthand()
        .ok_or(anyhow!("cannot create patch from detached head; aborting"))?;
    let head_branch = RefLike::try_from(head_branch)?;

    // Make sure the `HEAD` commit can be found in the monorepo. Otherwise there
    // is no way for anyone to merge this patch.
    let mut spinner = term::spinner(format!(
        "Looking for HEAD ({}) in storage...",
        term::format::secondary(common::fmt::oid(&head_oid))
    ));
    if storage.find_object(Oid::from(head_oid))?.is_none() {
        if !options.push {
            spinner.failed();
            term::blank();

            return Err(Error::WithHint {
                err: anyhow!("Current branch head was not found in storage"),
                hint: "hint: run `git push rad` and try again",
            }
            .into());
        }
        spinner.message("Pushing HEAD to storage...");

        let output = git::git(Path::new("."), ["push", "rad"])?;
        if options.verbose {
            spinner.finish();
            term::blob(output);
        }
    }
    spinner.finish();

    // Determine the merge target for this patch. This can ben any tracked remote's "default"
    // branch, as well as your own (eg. `rad/master`).
    let mut spinner = term::spinner("Analyzing remotes...");
    let targets = patch::find_merge_targets(&head_oid, storage, project)?;

    // eg. `refs/namespaces/<proj>/refs/remotes/<peer>/heads/master`
    let (target_peer, target_oid) = match targets.not_merged.as_slice() {
        [] => {
            spinner.message("All tracked peers are up to date.");
            return Ok(());
        }
        [target] => target,
        _ => {
            // TODO: Let user select which branch to use as a target.
            todo!();
        }
    };
    // TODO: Tell user how many peers don't have this change.
    spinner.finish();

    // TODO: Handle case where `rad/master` isn't up to date with the target.
    // In that case we should warn the user that their master branch is not up
    // to date, and error out, unless the user specifies manually the merge
    // base.

    // The merge base is basically the commit at which the histories diverge.
    let base_oid = repo.merge_base((*target_oid).into(), head_oid)?;
    let commits = patch::patch_commits(repo, &base_oid, &head_oid)?;

    let patch = match &options.update {
        Update::No => None,
        Update::Any => {
            let mut spinner = term::spinner("Finding patches to update...");
            let mut result = find_unmerged_with_base(
                head_oid,
                **target_oid,
                base_oid,
                &patches,
                &project.urn,
                repo,
            )?;

            if let Some((id, patch)) = result.pop() {
                if result.is_empty() {
                    spinner.message(format!(
                        "Found existing patch {} {}",
                        term::format::tertiary(common::fmt::cob(&id)),
                        term::format::italic(&patch.title)
                    ));
                    spinner.finish();
                    term::blank();

                    Some((id, patch))
                } else {
                    spinner.failed();
                    term::blank();
                    anyhow::bail!("More than one patch available to update, please specify an id with `rad patch --update <id>`");
                }
            } else {
                spinner.failed();
                term::blank();
                anyhow::bail!("No patches found that share a base, please create a new patch or specify the patch id manually");
            }
        }
        Update::Patch(identifier) => {
            if let Some((id, patch)) = patches.resolve(&project.urn, identifier)? {
                Some((id, patch))
            } else {
                anyhow::bail!("Patch '{}' not found", identifier);
            }
        }
    };

    if let Some((id, patch)) = patch {
        if term::confirm("Update?") {
            term::blank();

            return update(
                patch, id, &base_oid, &head_oid, &patches, project, repo, options, profile,
            );
        } else {
            anyhow::bail!("Patch update aborted by user");
        }
    }

    // TODO: List matching working copy refs for all targets.

    let user_name = storage.config_readonly()?.user_name()?;
    term::blank();
    term::info!(
        "{}/{} ({}) <- {}/{} ({})",
        target_peer.name(),
        term::format::highlight(&project.default_branch.to_string()),
        term::format::secondary(&common::fmt::oid(target_oid)),
        user_name,
        term::format::highlight(&head_branch.to_string()),
        term::format::secondary(&common::fmt::oid(&head_oid)),
    );

    // TODO: Test case where the target branch has been re-written passed the merge-base, since the fork was created
    // This can also happen *after* the patch is created.

    term::patch::print_commits_ahead_behind(repo, head_oid, (*target_oid).into())?;

    // List commits in patch that aren't in the target branch.
    term::blank();
    term::patch::list_commits(&commits)?;
    term::blank();

    if !term::confirm("Continue?") {
        anyhow::bail!("patch proposal aborted by user");
    }

    let message = head_commit
        .message()
        .ok_or(anyhow!("commit summary is not valid UTF-8; aborting"))?;
    let message = options.message.get(&format!("{}{}", message, PATCH_MSG));
    let (title, description) = message.split_once("\n\n").unwrap_or((&message, ""));
    let (title, description) = (title.trim(), description.trim());
    let description = description.replace(PATCH_MSG.trim(), ""); // Delete help message.

    if title.is_empty() {
        anyhow::bail!("a title must be given");
    }

    let title_pretty = &term::format::dim(format!("╭─ {} ───────", title));

    term::blank();
    term::print(title_pretty);
    term::blank();

    if description.is_empty() {
        term::print(term::format::italic("No description provided."));
    } else {
        term::markdown(&description);
    }

    term::blank();
    term::print(&term::format::dim(format!(
        "╰{}",
        "─".repeat(term::text_width(title_pretty) - 1)
    )));
    term::blank();

    if !term::confirm("Create patch?") {
        anyhow::bail!("patch proposal aborted by user");
    }

    let id = patches.create(
        &project.urn,
        title,
        &description,
        MergeTarget::default(),
        base_oid,
        head_oid,
        &[],
    )?;

    term::blank();
    term::success!("Patch {} created 🌱", term::format::highlight(id));

    if options.sync {
        let rt = tokio::runtime::Runtime::new()?;

        term::sync::sync(
            project.urn.clone(),
            sync::seeds(profile)?,
            sync::Mode::Push,
            profile,
            term::signer(profile)?,
            &rt,
        )?;
    }

    Ok(())
}

/// Create a human friendly message about git's sync status.
fn pretty_sync_status(
    repo: &git::Repository,
    revision_oid: git::Oid,
    head_oid: git::Oid,
) -> anyhow::Result<String> {
    let (a, b) = repo.graph_ahead_behind(revision_oid, head_oid)?;
    if a == 0 && b == 0 {
        return Ok(term::format::dim("up to date"));
    }

    let ahead = term::format::positive(a);
    let behind = term::format::negative(b);

    Ok(format!("ahead {}, behind {}", ahead, behind))
}

/// Make a human friendly string for commit version information.
///
/// For example '<oid> (branch1[, branch2])'.
fn pretty_commit_version(
    revision_oid: &git::Oid,
    repo: &Option<git::Repository>,
) -> anyhow::Result<String> {
    let mut oid = common::fmt::oid(revision_oid);
    let mut branches: Vec<String> = vec![];

    if let Some(repo) = repo {
        for r in repo.references()?.flatten() {
            if !r.is_branch() {
                continue;
            }
            if let (Some(oid), Some(name)) = (&r.target(), &r.shorthand()) {
                if oid == revision_oid {
                    branches.push(name.to_string());
                };
            };
        }
    };
    if !branches.is_empty() {
        oid = format!(
            "{} {}",
            term::format::secondary(oid),
            term::format::yellow(format!("({})", branches.join(", "))),
        );
    }

    Ok(oid)
}

/// Adds patch details as a new row to `table` and render later.
pub fn print(
    whoami: &LocalIdentity,
    patch_id: &PatchId,
    patch: &mut Patch,
    project: &project::Metadata,
    monorepo: &git::Repository,
    repo: &Option<git::Repository>,
    storage: &Storage,
) -> anyhow::Result<()> {
    for r in patch.revisions.iter_mut() {
        for (_, r) in &mut r.reviews {
            r.author.resolve(storage).ok();
        }
    }
    patch.author.resolve(storage).ok();

    let verified = project.verified(storage)?;
    let target_head = common::patch::patch_merge_target_oid(patch.target, verified, storage)?;

    let you = patch.author.urn() == &whoami.urn();
    let prefix = "└─ ";
    let mut author_info = vec![format!(
        "{}* opened by {}",
        prefix,
        term::format::tertiary(patch.author.name()),
    )];

    if you {
        author_info.push(term::format::secondary("(you)"));
    }
    author_info.push(term::format::dim(patch.timestamp));

    let revision = patch.revisions.last();
    term::info!(
        "{} {} {} {} {}",
        term::format::bold(&patch.title),
        term::format::highlight(common::fmt::cob(patch_id)),
        term::format::dim(format!("R{}", patch.version())),
        pretty_commit_version(&revision.oid, repo)?,
        pretty_sync_status(monorepo, *revision.oid, target_head)?,
    );
    term::info!("{}", author_info.join(" "));

    let mut timeline = Vec::new();
    for merge in &revision.merges {
        let peer = project::PeerInfo::get(&merge.peer, project, storage);
        let mut badges = Vec::new();

        if peer.delegate {
            badges.push(term::format::secondary("(delegate)"));
        }
        if peer.id == *storage.peer_id() {
            badges.push(term::format::secondary("(you)"));
        }

        timeline.push((
            merge.timestamp,
            format!(
                "{}{} by {} {}",
                " ".repeat(term::text_width(prefix)),
                term::format::secondary(term::format::dim("✓ merged")),
                term::format::tertiary(peer.name()),
                badges.join(" "),
            ),
        ));
    }
    for (_, review) in &revision.reviews {
        let verdict = match review.verdict {
            Some(Verdict::Accept) => term::format::positive(term::format::dim("✓ accepted")),
            Some(Verdict::Reject) => term::format::negative(term::format::dim("✗ rejected")),
            None => term::format::negative(term::format::dim("⋄ reviewed")),
        };
        let peer = project::PeerInfo::get(&review.author.peer, project, storage);
        let mut badges = Vec::new();

        if peer.delegate {
            badges.push(term::format::secondary("(delegate)"));
        }
        if peer.id == *storage.peer_id() {
            badges.push(term::format::secondary("(you)"));
        }

        timeline.push((
            review.timestamp,
            format!(
                "{}{} by {} {}",
                " ".repeat(term::text_width(prefix)),
                verdict,
                term::format::tertiary(review.author.name()),
                badges.join(" "),
            ),
        ));
    }
    timeline.sort_by_key(|(t, _)| *t);

    for (time, event) in timeline.iter().rev() {
        term::info!("{} {}", event, term::format::dim(time));
    }

    Ok(())
}

/// Find patches with a merge base equal to the one provided.
fn find_unmerged_with_base(
    patch_head: git::Oid,
    target_head: git::Oid,
    merge_base: git::Oid,
    patches: &PatchStore,
    project: &common::Urn,
    repo: &git::Repository,
) -> anyhow::Result<Vec<(PatchId, Patch)>> {
    // My patches.
    let proposed: Vec<_> = patches
        .proposed_by(patches.whoami.urn(), project)?
        .collect();

    let mut matches = Vec::new();

    for (id, patch) in proposed {
        let (_, rev) = patch.latest();

        if !rev.merges.is_empty() {
            continue;
        }
        if **patch.head() == patch_head {
            continue;
        }
        // Merge-base between the two patches.
        if repo.merge_base(**patch.head(), target_head)? == merge_base {
            matches.push((id, patch));
        }
    }
    Ok(matches)
}
