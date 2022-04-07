//! Patch-related functions and types.
use anyhow::{Error, Result};
use git2::Oid;

use librad::git::refs::Refs;
use librad::git::storage::{ReadOnly, ReadOnlyStorage};
use librad::PeerId;

use crate::{person, project};
use rad_terminal::components as term;

/// A patch can either be open or merged. It's considered merged, as soon as the
/// merge-base of the default's branch HEAD and the patch commit is the same than the
/// patch commit itself.
#[derive(PartialEq, Eq)]
pub enum State {
    Open,
    Merged,
}

/// A patch is a change set that a user wants the maintainer to merge into a projects default
/// branch.
///
/// A patch is represented by an annotated tag, prefixed with `radicle-patch/`.
pub struct Metadata {
    /// ID of a patch. This is the portion of the tag name following the `radicle-patch/` prefix.
    pub id: String,
    /// Peer that the patch originated from
    pub peer: PeerId,
    /// Message attached to the patch. This is the message of the annotated tag.
    pub message: Option<String>,
    /// Head commit that the author wants to merge with this patch.
    pub commit: Oid,
    /// The merge base of [`Metadata::commit`] and the head commit of the first maintainer's default
    /// branch.
    pub merge_base: Option<Oid>,
}

impl Metadata {
    pub fn state(&self) -> State {
        if self.merge_base.unwrap() == self.commit {
            return State::Merged;
        }
        State::Open
    }

    pub fn name(&self) -> String {
        str::replace(&self.id, "radicle-patch/", "")
    }
}

/// List patches on the local device. Returns a given peer's patches or this peer's
/// patches if `peer` is `None`.
pub fn list<S>(
    storage: &S,
    repo: &git2::Repository,
    project: &project::Metadata,
    peer: Option<PeerId>,
) -> Result<Vec<Metadata>, Error>
where
    S: AsRef<ReadOnly>,
{
    let storage = storage.as_ref();
    let mut patches: Vec<Metadata> = vec![];
    let master = repo
        .resolve_reference_from_short_name(&format!("rad/{}", project.default_branch))?
        .target()
        .unwrap();

    match Refs::load(&storage, &project.urn, peer) {
        Ok(refs) => {
            let blobs = match refs {
                Some(refs) => refs.tags().collect(),
                None => vec![],
            };
            for blob in blobs {
                let object = storage.find_object(blob.1)?.unwrap();
                let tag = object.peel_to_tag()?;
                let merge_base = repo.merge_base(master, tag.target_id())?;

                patches.push(Metadata {
                    id: tag.name().unwrap().to_string(),
                    peer: peer.unwrap_or(*storage.peer_id()),
                    message: Some(tag.message().unwrap().to_string()),
                    commit: tag.target_id(),
                    merge_base: Some(merge_base),
                });
            }
        }
        Err(_) => {}
    }

    Ok(patches)
}

/// Adds patch details as a new row to `table` and render later.
pub fn print<S>(
    project: &project::Metadata,
    storage: &S,
    patch: &Metadata,
    peer: Option<PeerId>,
    table: &mut term::Table<2>,
) -> anyhow::Result<()>
where
    S: AsRef<ReadOnly>,
{
    let storage = storage.as_ref();
    let peer_id = peer.unwrap_or(*storage.peer_id());

    if let Some(urn) = storage.config()?.user()? {
        if let Some(person) = person::get(&storage, &urn)? {
            if let Some(message) = patch.message.clone() {
                let you = peer_id == *storage.peer_id();
                let title = message.lines().next().unwrap_or("");
                let branch_info = format!(
                    "{} <= {}",
                    term::format::tertiary(project.default_branch.clone()),
                    term::format::tertiary(format!("{}/{}", peer_id, &patch.name())),
                );
                let mut author_info = vec![term::format::italic(format!(
                    "└── Opened by {}",
                    &person.subject().name
                ))];

                if you {
                    author_info.push(term::format::badge_secondary("you"));
                }

                table.push([term::format::bold(title), branch_info]);
                table.push([author_info.join(" "), "".to_owned()]);
            }
        }
    }
    Ok(())
}
