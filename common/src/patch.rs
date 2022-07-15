//! Patch-related functions and types.
use std::convert::TryInto;
use std::fmt;

use librad::git::identities;
use librad::git::identities::project::heads::DefaultBranchHead;
use librad::git::refs::Refs;
use librad::git::storage::{ReadOnly, ReadOnlyStorage};
use librad::git::{Storage, Urn};
use librad::PeerId;

use git_trailers as trailers;
use radicle_git_ext as git;
use serde::Serialize;

use crate::cobs::patch as cob;
use crate::project;

pub const TAG_PREFIX: &str = "patches/";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    #[error("storage: {0}")]
    Storage(#[from] librad::git::storage::Error),
}

/// A patch merge style.
#[derive(Debug, PartialEq, Eq)]
pub enum MergeStyle {
    /// A merge commit is created.
    Commit,
    /// The branch is fast-forwarded to the patch's commit.
    FastForward,
}

impl fmt::Display for MergeStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Commit => {
                write!(f, "merge-commit")
            }
            Self::FastForward => {
                write!(f, "fast-forward")
            }
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum State {
    Open,
    Merged,
}

/// A patch is a change set that a user wants the maintainer to merge into a project's default
/// branch.
///
/// A patch is represented by an annotated tag, prefixed with `patches/`.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    /// ID of a patch. This is the portion of the tag name following the `patches/` prefix.
    pub id: String,
    /// Peer that the patch originated from
    pub peer: project::PeerInfo,
    /// Message attached to the patch. This is the message of the annotated tag.
    pub message: Option<String>,
    /// Head commit that the author wants to merge with this patch.
    pub commit: git::Oid,
}

/// Tries to construct a patch from ['git2::Tag'] and ['project::PeerInfo'].
/// If the tag name matches the radicle patch prefix, a new patch metadata is
/// created.
pub fn from_tag(tag: git2::Tag, info: project::PeerInfo) -> Result<Option<Tag>, Error> {
    let patch = tag
        .name()
        .and_then(|name| name.strip_prefix(TAG_PREFIX))
        .map(|id| Tag {
            id: id.to_owned(),
            peer: info,
            message: tag.message().map(|m| m.to_string()),
            commit: tag.target_id().into(),
        });

    Ok(patch)
}

/// List patches on the local device. Returns a given peer's patches or this peer's
/// patches if `peer` is `None`.
pub fn all<S>(
    project: &project::Metadata,
    peer: Option<project::PeerInfo>,
    storage: &S,
) -> Result<Vec<Tag>, Error>
where
    S: AsRef<ReadOnly>,
{
    let storage = storage.as_ref();
    let mut patches: Vec<Tag> = vec![];

    let peer_id = peer.clone().map(|p| p.id);
    let info = match peer {
        Some(info) => info,
        None => project::PeerInfo::get(storage.peer_id(), project, storage),
    };

    if let Ok(refs) = Refs::load(&storage, &project.urn, peer_id) {
        let blobs = match refs {
            Some(refs) => refs.tags().collect(),
            None => vec![],
        };
        for (_, oid) in blobs {
            match storage.find_object(oid) {
                Ok(Some(object)) => {
                    let tag = object.peel_to_tag()?;

                    if let Some(patch) = from_tag(tag, info.clone())? {
                        patches.push(patch);
                    }
                }
                Ok(None) => {
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }
    }

    Ok(patches)
}

pub fn state(repo: &git2::Repository, patch: &Tag) -> State {
    match merge_base(repo, patch) {
        Ok(Some(merge_base)) => match merge_base == patch.commit {
            true => State::Merged,
            false => State::Open,
        },
        Ok(None) | Err(_) => State::Open,
    }
}

pub fn merge_base(repo: &git2::Repository, patch: &Tag) -> Result<Option<git::Oid>, Error> {
    let head = repo.head()?;
    let merge_base = match repo.merge_base(head.target().unwrap(), *patch.commit) {
        Ok(commit) => Some(commit),
        Err(_) => None,
    };

    Ok(merge_base.map(|o| o.into()))
}

pub fn is_merged(
    repo: &git2::Repository,
    target: git2::Oid,
    commit: git2::Oid,
) -> Result<bool, Error> {
    if let Ok(base) = repo.merge_base(target, commit) {
        Ok(base == commit)
    } else {
        Ok(false)
    }
}

/// Create a "patch" tag under:
///
/// > /refs/namespaces/<project>/refs/tags/patches/<patch>/<remote>/<revision>
///
pub fn create_tag(
    repo: &git2::Repository,
    author: &Urn,
    project: &Urn,
    patch_id: cob::PatchId,
    peer_id: &PeerId,
    commit: git2::Oid,
    revision: usize,
) -> Result<git2::Oid, Error> {
    let commit = repo.find_commit(commit)?;
    let name = format!("{patch_id}/{peer_id}/{revision}");
    let trailers = [
        trailers::Trailer {
            token: "Rad-Cob".try_into().unwrap(),
            values: vec![patch_id.to_string().into()],
        },
        trailers::Trailer {
            token: "Rad-Author".try_into().unwrap(),
            values: vec![author.to_string().into()],
        },
        trailers::Trailer {
            token: "Rad-Peer".try_into().unwrap(),
            values: vec![peer_id.to_string().into()],
        },
    ]
    .iter()
    .map(|t| t.display(": ").to_string())
    .collect::<Vec<_>>()
    .join("\n");

    repo.set_namespace(&project.to_string())?;

    let oid = repo.tag(
        &name,
        commit.as_object(),
        &repo.signature()?,
        &trailers,
        false,
    )?;

    Ok(oid)
}

#[derive(Debug, Default)]
pub struct MergeTargets {
    pub merged: Vec<project::PeerInfo>,
    pub not_merged: Vec<(project::PeerInfo, git::Oid)>,
}

pub fn find_merge_targets<S>(
    head: &git2::Oid,
    storage: &S,
    project: &project::Metadata,
) -> anyhow::Result<MergeTargets>
where
    S: AsRef<ReadOnly>,
{
    let mut targets = MergeTargets::default();
    let storage = storage.as_ref();
    let repo = git2::Repository::open_bare(storage.path())?;

    for (peer_id, peer_info) in project::tracked(project, storage)? {
        let target = project.remote_head(&peer_id);
        let target_oid = storage.reference_oid(&target)?;

        if is_merged(&repo, target_oid.into(), *head)? {
            targets.merged.push(peer_info);
        } else {
            targets.not_merged.push((peer_info, target_oid));
        }
    }
    Ok(targets)
}

pub fn patch_merge_target_oid(
    target: cob::MergeTarget,
    project: identities::VerifiedProject,
    storage: &Storage,
) -> anyhow::Result<git2::Oid> {
    let urn = project.urn();

    match target {
        cob::MergeTarget::Upstream => {
            if let DefaultBranchHead::Head { target, .. } =
                identities::project::heads::default_branch_head(storage, project)?
            {
                Ok(target)
            } else {
                anyhow::bail!(
                    "failed to determine default branch head for project {}",
                    urn,
                );
            }
        }
    }
}

/// Return commits between the merge base and a head.
pub fn patch_commits<'a>(
    repo: &'a git2::Repository,
    base: &git2::Oid,
    head: &git2::Oid,
) -> anyhow::Result<Vec<git2::Commit<'a>>> {
    let mut commits = Vec::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_range(&format!("{}..{}", base, head))?;

    for rev in revwalk {
        let commit = repo.find_commit(rev?)?;
        commits.push(commit);
    }

    Ok(commits)
}
