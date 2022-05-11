//! Patch-related functions and types.
use git2::Oid;

use librad::git::refs::Refs;
use librad::git::storage::ReadOnly;
use librad::git::storage::ReadOnlyStorage;
use serde::Serialize;

use crate::project;

pub const TAG_PREFIX: &str = "patches/";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    #[error("storage: {0}")]
    Storage(#[from] librad::git::storage::Error),
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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// ID of a patch. This is the portion of the tag name following the `patches/` prefix.
    pub id: String,
    /// Peer that the patch originated from
    pub peer: project::PeerInfo,
    /// Message attached to the patch. This is the message of the annotated tag.
    pub message: Option<String>,
    /// Head commit that the author wants to merge with this patch.
    #[serde(with = "string")]
    pub commit: Oid,
}

/// Tries to construct a patch from ['git2::Tag'] and ['project::PeerInfo'].
/// If the tag name matches the radicle patch prefix, a new patch metadata is
/// created.
pub fn from_tag(tag: git2::Tag, info: project::PeerInfo) -> Result<Option<Metadata>, Error> {
    let patch = tag
        .name()
        .and_then(|name| name.strip_prefix(TAG_PREFIX))
        .map(|id| Metadata {
            id: id.to_owned(),
            peer: info,
            message: tag.message().map(|m| m.to_string()),
            commit: tag.target_id(),
        });

    Ok(patch)
}

/// List patches on the local device. Returns a given peer's patches or this peer's
/// patches if `peer` is `None`.
pub fn all<S>(
    project: &project::Metadata,
    peer: Option<project::PeerInfo>,
    storage: &S,
) -> Result<Vec<Metadata>, Error>
where
    S: AsRef<ReadOnly>,
{
    let storage = storage.as_ref();
    let mut patches: Vec<Metadata> = vec![];

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

pub fn state(repo: &git2::Repository, patch: &Metadata) -> State {
    match merge_base(repo, patch) {
        Ok(Some(merge_base)) => match merge_base == patch.commit {
            true => State::Merged,
            false => State::Open,
        },
        Ok(None) | Err(_) => State::Open,
    }
}

pub fn merge_base(repo: &git2::Repository, patch: &Metadata) -> Result<Option<Oid>, Error> {
    let head = repo.head()?;
    let merge_base = match repo.merge_base(head.target().unwrap(), patch.commit) {
        Ok(commit) => Some(commit),
        Err(_) => None,
    };

    Ok(merge_base)
}

mod string {
    use std::fmt::Display;

    use serde::Serializer;

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }
}
