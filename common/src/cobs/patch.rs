#![allow(clippy::too_many_arguments)]
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::ops::{ControlFlow, RangeInclusive};
use std::str::FromStr;

use automerge::transaction::Transactable;
use automerge::{Automerge, AutomergeError, ObjType, ScalarValue, Value};
use lazy_static::lazy_static;
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use librad::collaborative_objects::{
    CollaborativeObjects, EntryContents, History, NewObjectSpec, ObjectId, TypeName,
    UpdateObjectSpec,
};
use librad::git::identities::local::LocalIdentity;
use librad::git::Storage;
use librad::git::Urn;
use librad::paths::Paths;
use librad::PeerId;

use radicle_git_ext as git;

use crate::cobs::shared;
use crate::cobs::shared::*;

lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.patch").unwrap();
    pub static ref SCHEMA: serde_json::Value =
        serde_json::from_slice(include_bytes!("patch.json")).unwrap();
}

/// Identifier for a patch.
pub type PatchId = ObjectId;

/// Identifier for a revision.
pub type RevisionId = usize;

/// Where a patch is intended to be merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeTarget {
    /// Intended for the default branch of the project delegates.
    /// Note that if the delegations change while the patch is open,
    /// this will always mean whatever the "current" delegation set is.
    Upstream,
}

impl Default for MergeTarget {
    fn default() -> Self {
        Self::Upstream
    }
}

impl From<MergeTarget> for ScalarValue {
    fn from(target: MergeTarget) -> Self {
        match target {
            MergeTarget::Upstream => ScalarValue::from("upstream"),
        }
    }
}

impl<'a> TryFrom<Value<'a>> for MergeTarget {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let state = value.to_str().ok_or("value isn't a string")?;

        match state {
            "upstream" => Ok(Self::Upstream),
            _ => Err("invalid merge target type"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Create error: {0}")]
    Create(String),

    #[error("Update error: {0}")]
    Update(String),

    #[error("List error: {0}")]
    List(String),

    #[error("Retrieve error: {0}")]
    Retrieve(String),

    #[error(transparent)]
    Automerge(#[from] AutomergeError),
}

#[derive(Debug, Clone, Serialize)]
pub struct Patch {
    /// Author of the patch.
    pub author: Author,
    /// Peer who authored the patch.
    pub peer: PeerId,
    /// Title of the patch.
    pub title: String,
    /// Current state of the patch.
    pub state: State,
    /// Target this patch is meant to be merged in.
    pub target: MergeTarget,
    /// Labels associated with the patch.
    pub labels: HashSet<Label>,
    /// List of patch revisions. The initial changeset is part of the
    /// first revision.
    pub revisions: NonEmpty<Revision>,
    /// Patch creation time.
    pub timestamp: Timestamp,
}

impl Patch {
    pub fn head(&self) -> &git::Oid {
        &self.revisions.last().oid
    }

    pub fn version(&self) -> RevisionId {
        self.revisions.len() - 1
    }

    pub fn latest(&self) -> (RevisionId, &Revision) {
        let version = self.version();
        let revision = &self.revisions[version];

        (version, revision)
    }

    pub fn is_proposed(&self) -> bool {
        matches!(self.state, State::Proposed)
    }

    pub fn is_archived(&self) -> bool {
        matches!(self.state, State::Archived)
    }

    pub fn description(&self) -> &str {
        self.latest().1.description()
    }
}

impl TryFrom<Automerge> for Patch {
    type Error = AutomergeError;

    fn try_from(doc: Automerge) -> Result<Self, Self::Error> {
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "patch")?.unwrap();
        let (title, _) = doc.get(&obj_id, "title")?.unwrap();
        let (author, _) = doc.get(&obj_id, "author")?.unwrap();
        let (peer, _) = doc.get(&obj_id, "peer")?.unwrap();
        let (state, _) = doc.get(&obj_id, "state")?.unwrap();
        let (target, _) = doc.get(&obj_id, "target")?.unwrap();
        let (timestamp, _) = doc.get(&obj_id, "timestamp")?.unwrap();
        let (labels, labels_id) = doc.get(&obj_id, "labels")?.unwrap();

        assert_eq!(labels.to_objtype(), Some(ObjType::Map));

        let mut revisions = Vec::new();
        let (_, revisions_id) = doc.get(&obj_id, "revisions")?.unwrap();
        for i in 0..doc.length(&revisions_id) {
            let revision = lookup::revision(&doc, &revisions_id, i).unwrap();
            revisions.push(revision);
        }

        // Labels.
        let mut labels = HashSet::new();
        for key in doc.keys(&labels_id) {
            let label = Label::new(key).unwrap();

            labels.insert(label);
        }

        let author = Author::from_value(author)?;
        let peer = PeerId::from_value(peer)?;
        let state = State::try_from(state).unwrap();
        let revisions = NonEmpty::from_vec(revisions).unwrap();
        let target = MergeTarget::try_from(target).unwrap();
        let timestamp = Timestamp::try_from(timestamp).unwrap();

        Ok(Self {
            author,
            peer,
            title: title.into_string().unwrap(),
            state,
            target,
            labels,
            revisions,
            timestamp,
        })
    }
}

impl TryFrom<&History> for Patch {
    type Error = anyhow::Error;

    fn try_from(history: &History) -> Result<Self, Self::Error> {
        let doc = history.traverse(Automerge::new(), |mut doc, entry| {
            match entry.contents() {
                EntryContents::Automerge(bytes) => {
                    match automerge::Change::from_bytes(bytes.clone()) {
                        Ok(change) => {
                            doc.apply_changes([change]).ok();
                        }
                        Err(_err) => {
                            // Ignore
                        }
                    }
                }
            }
            ControlFlow::Continue(doc)
        });
        let patch = Patch::try_from(doc)?;

        Ok(patch)
    }
}

pub struct Patches<'a> {
    pub whoami: LocalIdentity,
    pub peer_id: PeerId,

    store: CollaborativeObjects<'a>,
}

impl<'a> Patches<'a> {
    pub fn new(whoami: LocalIdentity, paths: &Paths, storage: &'a Storage) -> Result<Self, Error> {
        let store = storage.collaborative_objects(Some(paths.cob_cache_dir().to_path_buf()));
        let peer_id = *storage.peer_id();

        Ok(Self {
            store,
            whoami,
            peer_id,
        })
    }

    pub fn create(
        &self,
        project: &Urn,
        title: &str,
        description: &str,
        target: MergeTarget,
        oid: impl Into<git::Oid>,
        labels: &[Label],
    ) -> Result<PatchId, Error> {
        let author = self.whoami.urn();
        let timestamp = Timestamp::now();
        let revision = Revision::new(
            author.clone(),
            self.peer_id,
            oid.into(),
            description.to_owned(),
            timestamp,
        );
        let history = events::create(
            &author,
            &self.peer_id,
            title,
            &revision,
            target,
            timestamp,
            labels,
        )?;

        cobs::create(history, project, &self.whoami, &self.store)
    }

    pub fn update(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        comment: impl ToString,
        oid: impl Into<git::Oid>,
    ) -> Result<RevisionId, Error> {
        let author = self.whoami.urn();
        let timestamp = Timestamp::now();
        let revision = Revision::new(
            author,
            self.peer_id,
            oid.into(),
            comment.to_string(),
            timestamp,
        );

        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let (revision_id, changes) = events::update(&mut patch, revision)?;

        cobs::update(
            *patch_id,
            project,
            "Update patch",
            changes,
            &self.whoami,
            &self.store,
        )?;

        Ok(revision_id)
    }

    pub fn get(&self, project: &Urn, id: &PatchId) -> Result<Option<Patch>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        if let Some(cob) = cob {
            let patch = Patch::try_from(cob.history()).unwrap();
            Ok(Some(patch))
        } else {
            Ok(None)
        }
    }

    pub fn get_raw(&self, project: &Urn, id: &PatchId) -> Result<Option<Automerge>, Error> {
        let cob = self
            .store
            .retrieve(project, &TYPENAME, id)
            .map_err(|e| Error::Retrieve(e.to_string()))?;

        let cob = if let Some(cob) = cob {
            cob
        } else {
            return Ok(None);
        };

        let doc = cob.history().traverse(Vec::new(), |mut doc, entry| {
            match entry.contents() {
                EntryContents::Automerge(bytes) => {
                    doc.extend(bytes);
                }
            }
            ControlFlow::Continue(doc)
        });

        let doc = Automerge::load(&doc)?;

        Ok(Some(doc))
    }

    pub fn merge(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        revision: RevisionId,
        commit: git::Oid,
    ) -> Result<Merge, Error> {
        let timestamp = Timestamp::now();
        let merge = Merge {
            peer: self.peer_id,
            commit,
            timestamp,
        };

        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let changes = events::merge(&mut patch, revision, &merge)?;

        cobs::update(
            *patch_id,
            project,
            "Merge revision",
            changes,
            &self.whoami,
            &self.store,
        )?;

        Ok(merge)
    }

    pub fn find(
        &self,
        project: &Urn,
        predicate: impl Fn(&PatchId) -> bool,
    ) -> Result<Vec<PatchId>, Error> {
        let cobs = self
            .store
            .list(project, &TYPENAME)
            .map_err(|e| Error::List(e.to_string()))?;

        Ok(cobs
            .into_iter()
            .map(|c| *c.id())
            .filter(|id| predicate(id))
            .collect())
    }

    pub fn all(&self, project: &Urn) -> Result<Vec<(PatchId, Patch)>, Error> {
        let cobs = self
            .store
            .list(project, &TYPENAME)
            .map_err(|e| Error::List(e.to_string()))?;

        let mut patches = Vec::new();
        for cob in cobs {
            let patch: Result<Patch, _> = cob.history().try_into();
            patches.push((*cob.id(), patch.unwrap()));
        }
        patches.sort_by_key(|(_, p)| p.timestamp);

        Ok(patches)
    }

    pub fn proposed(&self, project: &Urn) -> Result<impl Iterator<Item = (PatchId, Patch)>, Error> {
        let all = self.all(project)?;

        Ok(all.into_iter().filter(|(_, p)| p.is_proposed()))
    }

    pub fn proposed_by(
        &self,
        who: Urn,
        project: &Urn,
    ) -> Result<impl Iterator<Item = (PatchId, Patch)>, Error> {
        Ok(self
            .proposed(project)?
            .filter(move |(_, p)| p.author.urn() == &who))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Draft,
    Proposed,
    Archived,
}

impl From<State> for ScalarValue {
    fn from(state: State) -> Self {
        match state {
            State::Proposed => ScalarValue::from("proposed"),
            State::Draft => ScalarValue::from("draft"),
            State::Archived => ScalarValue::from("archived"),
        }
    }
}

impl<'a> TryFrom<Value<'a>> for State {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let state = value.to_str().ok_or("value isn't a string")?;

        match state {
            "proposed" => Ok(Self::Proposed),
            "draft" => Ok(Self::Draft),
            "archived" => Ok(Self::Archived),
            _ => Err("invalid state name"),
        }
    }
}

/// A patch revision.
#[derive(Debug, Clone, Serialize)]
pub struct Revision {
    /// Author of this revision.
    /// Note that this doesn't have to match the author of the patch.
    pub author: Author,
    /// Peer who published this revision.
    pub peer: PeerId,
    /// Reference to the Git object containing the code.
    pub oid: git::Oid,
    /// "Cover letter" for this changeset.
    pub comment: Comment,
    /// Discussion around this revision.
    pub discussion: Discussion,
    /// Reviews (one per user) of the changes.
    pub reviews: HashMap<Urn, Review>,
    /// Merges of this revision into other repositories.
    pub merges: Vec<Merge>,
    /// When this revision was created.
    pub timestamp: Timestamp,
}

impl Revision {
    pub fn new(
        author: Urn,
        peer: PeerId,
        oid: git::Oid,
        comment: String,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            author: Author::from(author.clone()),
            peer,
            oid,
            comment: Comment::new(author, comment, timestamp),
            discussion: Discussion::default(),
            reviews: HashMap::default(),
            merges: Vec::default(),
            timestamp,
        }
    }

    pub fn description(&self) -> &str {
        &self.comment.body
    }

    /// Put this object into an automerge document.
    fn put(
        &self,
        tx: &mut automerge::transaction::Transaction,
        id: &automerge::ObjId,
    ) -> Result<(), AutomergeError> {
        tx.put(&id, "author", self.author.urn().to_string())?;
        tx.put(&id, "peer", self.peer.to_string())?;
        tx.put(&id, "oid", self.oid.to_string())?;
        {
            // Top-level comment for first patch revision.
            // Nb. top-level comment doesn't have a `replies` field.
            let comment_id = tx.put_object(&id, "comment", ObjType::Map)?;

            tx.put(&comment_id, "body", self.comment.body.trim())?;
            tx.put(&comment_id, "author", self.comment.author.urn().to_string())?;
            tx.put(&comment_id, "timestamp", self.comment.timestamp)?;
            tx.put_object(&comment_id, "reactions", ObjType::Map)?;
        }
        tx.put_object(&id, "discussion", ObjType::List)?;
        tx.put_object(&id, "reviews", ObjType::Map)?;
        tx.put_object(&id, "merges", ObjType::List)?;
        tx.put(&id, "timestamp", self.timestamp)?;

        Ok(())
    }
}

/// A merged patch revision.
#[derive(Debug, Clone, Serialize)]
pub struct Merge {
    /// Peer id of repository that this patch was merged into.
    pub peer: PeerId,
    /// Base branch commit that contains the revision.
    pub commit: git::Oid,
    /// When this merged was performed.
    pub timestamp: Timestamp,
}

/// A patch review verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Accept patch.
    Accept,
    /// Reject patch.
    Reject,
    /// Don't give a verdict.
    Pass,
}

impl From<Verdict> for ScalarValue {
    fn from(verdict: Verdict) -> Self {
        let s = serde_json::to_string(&verdict).unwrap(); // Cannot fail.
        ScalarValue::from(s)
    }
}

impl<'a> TryFrom<Value<'a>> for Verdict {
    type Error = serde_json::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let verdict = value
            .to_str()
            .ok_or(serde::de::Error::custom("value is not a string"))?;
        serde_json::from_str(verdict)
    }
}

/// Code location, used for attaching comments.
#[derive(Debug, Clone, Serialize)]
pub struct CodeLocation {
    /// Line number commented on.
    pub lines: RangeInclusive<usize>,
    /// Commit commented on.
    pub commit: git::Oid,
    /// File being commented on.
    pub blob: git::Oid,
}

/// Comment on code.
#[derive(Debug, Clone, Serialize)]
pub struct CodeComment {
    /// Code location of the comment.
    location: CodeLocation,
    /// Comment.
    comment: Comment,
}

/// A patch review on a revision.
#[derive(Debug, Clone, Serialize)]
pub struct Review {
    /// Review author.
    pub author: Author,
    /// Review verdict.
    pub verdict: Verdict,
    /// Review general comment.
    pub comment: Comment,
    /// Review inline code comments.
    pub inline: Vec<CodeComment>,
    /// Review timestamp.
    pub timestamp: Timestamp,
}

mod lookup {
    use super::*;

    pub fn revision(
        doc: &Automerge,
        revisions_id: &automerge::ObjId,
        id: RevisionId,
    ) -> Result<Revision, AutomergeError> {
        let (_, revision_id) = doc.get(&revisions_id, id)?.unwrap();
        let (_, comment_id) = doc.get(&revision_id, "comment")?.unwrap();
        let (_, discussion_id) = doc.get(&revision_id, "discussion")?.unwrap();
        let (_, _reviews_id) = doc.get(&revision_id, "reviews")?.unwrap();
        let (_, merges_id) = doc.get(&revision_id, "merges")?.unwrap();
        let (author, _) = doc.get(&revision_id, "author")?.unwrap();
        let (peer, _) = doc.get(&revision_id, "peer")?.unwrap();
        let (oid, _) = doc.get(&revision_id, "oid")?.unwrap();
        let (timestamp, _) = doc.get(&revision_id, "timestamp")?.unwrap();

        // Top-level comment.
        let comment = shared::lookup::comment(doc, &comment_id)?;

        // Discussion thread.
        let mut discussion: Discussion = Vec::new();
        for i in 0..doc.length(&discussion_id) {
            let (_, comment_id) = doc.get(&discussion_id, i as usize)?.unwrap();
            let comment = shared::lookup::thread(doc, &comment_id)?;

            discussion.push(comment);
        }

        // Patch merges.
        let mut merges: Vec<Merge> = Vec::new();
        for i in 0..doc.length(&merges_id) {
            let (_, merge_id) = doc.get(&merges_id, i as usize)?.unwrap();
            let merge = self::merge(doc, &merge_id)?;

            merges.push(merge);
        }

        let author = Author::from_value(author)?;
        let peer = PeerId::from_value(peer)?;
        let oid = oid.to_str().unwrap().try_into().unwrap();
        let reviews = HashMap::new();
        let timestamp = Timestamp::try_from(timestamp).unwrap();

        Ok(Revision {
            author,
            peer,
            oid,
            comment,
            discussion,
            reviews,
            merges,
            timestamp,
        })
    }

    pub fn merge(doc: &Automerge, obj_id: &automerge::ObjId) -> Result<Merge, AutomergeError> {
        let (peer, _) = doc.get(&obj_id, "peer")?.unwrap();
        let (commit, _) = doc.get(&obj_id, "commit")?.unwrap();
        let (timestamp, _) = doc.get(&obj_id, "timestamp")?.unwrap();

        let peer = PeerId::from_value(peer)?;
        let commit = git::Oid::from_str(&commit.into_string().unwrap()).unwrap();
        let timestamp = Timestamp::try_from(timestamp).unwrap();

        Ok(Merge {
            peer,
            commit,
            timestamp,
        })
    }
}

mod cobs {
    use super::*;

    pub(super) fn create(
        history: EntryContents,
        project: &Urn,
        whoami: &LocalIdentity,
        store: &CollaborativeObjects,
    ) -> Result<PatchId, Error> {
        let cob = store
            .create(
                whoami,
                project,
                NewObjectSpec {
                    schema_json: SCHEMA.clone(),
                    typename: TYPENAME.clone(),
                    message: Some("Create patch".to_owned()),
                    history,
                },
            )
            .map_err(|e| Error::Create(e.to_string()))?;

        Ok(*cob.id())
    }

    pub(super) fn update(
        object_id: PatchId,
        project: &Urn,
        message: &'static str,
        changes: EntryContents,
        whoami: &LocalIdentity,
        store: &CollaborativeObjects,
    ) -> Result<PatchId, Error> {
        let cob = store
            .update(
                whoami,
                project,
                UpdateObjectSpec {
                    object_id,
                    typename: TYPENAME.clone(),
                    message: Some(message.to_owned()),
                    changes,
                },
            )
            .map_err(|e| Error::Update(e.to_string()))?;

        Ok(*cob.id())
    }
}

mod events {
    use super::*;
    use automerge::{
        transaction::{CommitOptions, Transactable},
        ObjId,
    };

    pub fn create(
        author: &Urn,
        peer: &PeerId,
        title: &str,
        revision: &Revision,
        target: MergeTarget,
        timestamp: Timestamp,
        labels: &[Label],
    ) -> Result<EntryContents, AutomergeError> {
        let title = title.trim();
        // TODO: Return error.
        if title.is_empty() {
            panic!("Empty patch title");
        }

        let mut doc = Automerge::new();
        let _patch = doc
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Create patch".to_owned()),
                |tx| {
                    let patch_id = tx.put_object(ObjId::Root, "patch", ObjType::Map)?;

                    tx.put(&patch_id, "title", title)?;
                    tx.put(&patch_id, "author", author.to_string())?;
                    tx.put(&patch_id, "peer", peer.to_string())?;
                    tx.put(&patch_id, "state", State::Proposed)?;
                    tx.put(&patch_id, "target", target)?;
                    tx.put(&patch_id, "timestamp", timestamp)?;

                    let labels_id = tx.put_object(&patch_id, "labels", ObjType::Map)?;
                    for label in labels {
                        tx.put(&labels_id, label.name().trim(), true)?;
                    }

                    let revisions_id = tx.put_object(&patch_id, "revisions", ObjType::List)?;
                    let revision_id = tx.insert_object(&revisions_id, 0, ObjType::Map)?;

                    revision.put(tx, &revision_id)?;

                    Ok(patch_id)
                },
            )
            .map_err(|failure| failure.error)?
            .result;

        Ok(EntryContents::Automerge(doc.save_incremental()))
    }

    pub fn update(
        patch: &mut Automerge,
        revision: Revision,
    ) -> Result<(RevisionId, EntryContents), AutomergeError> {
        let success = patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Merge revision".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();

                    let length = tx.length(&revisions_id);
                    let revision_id = tx.insert_object(&revisions_id, length, ObjType::Map)?;

                    revision.put(tx, &revision_id)?;

                    Ok(length)
                },
            )
            .map_err(|failure| failure.error)?;

        let revision_id = success.result;
        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok((revision_id, EntryContents::Automerge(change)))
    }

    pub fn merge(
        patch: &mut Automerge,
        revision: RevisionId,
        merge: &Merge,
    ) -> Result<EntryContents, AutomergeError> {
        patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Merge revision".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();
                    let (_, revision_id) = tx.get(&revisions_id, revision)?.unwrap();
                    let (_, merges_id) = tx.get(&revision_id, "merges")?.unwrap();

                    let length = tx.length(&merges_id);
                    let merge_id = tx.insert_object(&merges_id, length, ObjType::Map)?;

                    tx.put(&merge_id, "peer", merge.peer.to_string())?;
                    tx.put(&merge_id, "commit", merge.commit.to_string())?;
                    tx.put(&merge_id, "timestamp", merge.timestamp)?;

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test;

    #[test]
    fn test_patch_create_and_get() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let author = whoami.urn();
        let timestamp = Timestamp::now();
        let patches = Patches::new(whoami, profile.paths(), &storage).unwrap();
        let target = MergeTarget::Upstream;
        let oid = git::Oid::from(git2::Oid::zero());
        let patch_id = patches
            .create(
                &project.urn(),
                "My first patch",
                "Blah blah blah.",
                target,
                oid,
                &[],
            )
            .unwrap();
        let patch = patches.get(&project.urn(), &patch_id).unwrap().unwrap();

        assert_eq!(&patch.title, "My first patch");
        assert_eq!(patch.author.urn(), &author);
        assert_eq!(&patch.peer, storage.peer_id());
        assert_eq!(patch.state, State::Proposed);
        assert!(patch.timestamp >= timestamp);

        let revision = patch.revisions.head;

        assert_eq!(revision.author, Author::Urn { urn: author });
        assert_eq!(revision.peer, *storage.peer_id());
        assert_eq!(revision.comment.body, "Blah blah blah.");
        assert_eq!(revision.discussion.len(), 0);
        assert_eq!(revision.oid, oid);
        assert!(revision.reviews.is_empty());
        assert!(revision.merges.is_empty());
    }

    #[test]
    fn test_patch_merge() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let patches = Patches::new(whoami, profile.paths(), &storage).unwrap();
        let target = MergeTarget::Upstream;
        let oid = git::Oid::from(git2::Oid::zero());
        let base = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let patch_id = patches
            .create(
                &project.urn(),
                "My first patch",
                "Blah blah blah.",
                target,
                oid,
                &[],
            )
            .unwrap();

        let _merge = patches.merge(&project.urn(), &patch_id, 0, base).unwrap();
        let patch = patches.get(&project.urn(), &patch_id).unwrap().unwrap();
        let merges = patch.revisions.head.merges;

        assert_eq!(merges.len(), 1);
        assert_eq!(merges[0].peer, *storage.peer_id());
        assert_eq!(merges[0].commit, base);
    }

    #[test]
    fn test_patch_update() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let patches = Patches::new(whoami, profile.paths(), &storage).unwrap();
        let target = MergeTarget::Upstream;
        let rev0_oid = git::Oid::from_str("518d5069f94c03427f694bb494ac1cd7d1339380").unwrap();
        let rev1_oid = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let project = &project.urn();
        let patch_id = patches
            .create(
                project,
                "My first patch",
                "Blah blah blah.",
                target,
                rev0_oid,
                &[],
            )
            .unwrap();

        let patch = patches.get(project, &patch_id).unwrap().unwrap();
        assert_eq!(patch.description(), "Blah blah blah.");
        assert_eq!(patch.version(), 0);

        let revision_id = patches
            .update(project, &patch_id, "I've made changes.", rev1_oid)
            .unwrap();

        assert_eq!(revision_id, 1);

        let patch = patches.get(project, &patch_id).unwrap().unwrap();
        assert_eq!(patch.description(), "I've made changes.");

        assert_eq!(patch.revisions.len(), 2);
        assert_eq!(patch.version(), 1);

        let (id, revision) = patch.latest();

        assert_eq!(id, 1);
        assert_eq!(revision.oid, rev1_oid);
        assert_eq!(revision.description(), "I've made changes.");
    }
}
