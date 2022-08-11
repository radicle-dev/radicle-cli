#![allow(clippy::too_many_arguments)]
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::ops::{ControlFlow, Deref, RangeInclusive};
use std::str::FromStr;
use std::sync::Arc;

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
use librad::git::storage::ReadOnly;
use librad::git::Urn;
use librad::PeerId;

use radicle_git_ext as git;

use crate::cobs::shared;
use crate::cobs::shared::*;

lazy_static! {
    pub static ref TYPENAME: TypeName = FromStr::from_str("xyz.radicle.patch").unwrap();
}

/// Identifier for a patch.
pub type PatchId = ObjectId;

/// Unique identifier for a revision.
pub type RevisionId = uuid::Uuid;

/// Index of a revision in the revisions list.
pub type RevisionIx = usize;

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

impl<'a> FromValue<'a> for MergeTarget {
    fn from_value(value: Value<'a>) -> Result<Self, ValueError> {
        let state = value.to_str().ok_or(ValueError::InvalidType)?;

        match state {
            "upstream" => Ok(Self::Upstream),
            _ => Err(ValueError::InvalidValue(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Patch<T = (), P = PeerId>
where
    T: Clone,
    P: Clone,
{
    /// Author of the patch.
    pub author: Author,
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
    pub revisions: NonEmpty<Revision<T, P>>,
    /// Patch creation time.
    pub timestamp: Timestamp,
}

impl Patch {
    pub fn head(&self) -> &git::Oid {
        &self.revisions.last().oid
    }

    pub fn version(&self) -> RevisionIx {
        self.revisions.len() - 1
    }

    pub fn latest(&self) -> (RevisionIx, &Revision) {
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

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.author.resolve(storage)?;

        for revision in &mut self.revisions.iter_mut() {
            revision.resolve(storage)?;
        }

        Ok(())
    }
}

impl Cob for Patch {
    fn type_name() -> &'static TypeName {
        &TYPENAME
    }

    fn from_history(history: &History) -> Result<Self, anyhow::Error> {
        Patch::try_from(history)
    }
}

impl TryFrom<Document<'_>> for Patch {
    type Error = DocumentError;

    fn try_from(doc: Document) -> Result<Self, Self::Error> {
        let (_obj, obj_id) = doc.get(automerge::ObjId::Root, "patch")?;
        let title = doc.val(&obj_id, "title")?;
        let author = doc.val(&obj_id, "author")?;
        let peer = doc.val(&obj_id, "peer")?;
        let state = doc.val(&obj_id, "state")?;
        let target = doc.val(&obj_id, "target")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;

        let revisions = doc.list(&obj_id, "revisions", lookup::revision)?;
        let labels: HashSet<Label> = doc.keys(&obj_id, "labels")?;
        let revisions = NonEmpty::from_vec(revisions).ok_or(DocumentError::EmptyList)?;
        let author: Author = Author::new(author, peer);

        Ok(Self {
            author,
            title,
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
        let patch = Patch::try_from(Document::new(&doc))?;

        Ok(patch)
    }
}

pub struct PatchStore<'a> {
    store: &'a Store<'a>,
}

impl<'a> Deref for PatchStore<'a> {
    type Target = Store<'a>;

    fn deref(&self) -> &Self::Target {
        self.store
    }
}

impl<'a> PatchStore<'a> {
    pub fn new(store: &'a Store<'a>) -> Self {
        Self { store }
    }

    pub fn resolve_id(
        &self,
        project: &Urn,
        identifier: &Identifier,
    ) -> anyhow::Result<Option<ObjectId>> {
        self.store.resolve_id::<Patch>(project, identifier)
    }

    pub fn create(
        &self,
        project: &Urn,
        title: &str,
        description: &str,
        target: MergeTarget,
        base: impl Into<git::Oid>,
        oid: impl Into<git::Oid>,
        labels: &[Label],
    ) -> Result<PatchId, Error> {
        let author = self.author();
        let timestamp = Timestamp::now();
        let revision = Revision::new(
            author.clone(),
            self.peer_id,
            base.into(),
            oid.into(),
            description.to_owned(),
            timestamp,
        );
        let history = events::create(&author, title, &revision, target, timestamp, labels)?;

        cobs::create(history, project, &self.whoami, self.store)
    }

    pub fn comment(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        revision_ix: RevisionIx,
        body: &str,
    ) -> Result<PatchId, Error> {
        let author = self.author();
        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let timestamp = Timestamp::now();
        let changes = events::comment(&mut patch, revision_ix, &author, body, timestamp)?;
        let cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *patch_id,
                    typename: TYPENAME.clone(),
                    message: Some("Add comment".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(*cob.id()) // TODO: Return something other than doc id.
    }

    pub fn update(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        comment: impl ToString,
        base: impl Into<git::Oid>,
        oid: impl Into<git::Oid>,
    ) -> Result<RevisionIx, Error> {
        let author = self.author();
        let timestamp = Timestamp::now();
        let revision = Revision::new(
            author,
            self.peer_id,
            base.into(),
            oid.into(),
            comment.to_string(),
            timestamp,
        );

        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let (revision_ix, changes) = events::update(&mut patch, revision)?;

        cobs::update(
            *patch_id,
            project,
            "Update patch",
            changes,
            &self.whoami,
            self.store,
        )?;

        Ok(revision_ix)
    }

    pub fn reply(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        revision_ix: RevisionIx,
        comment_id: CommentId,
        reply: &str,
    ) -> Result<(), Error> {
        let author = self.author();
        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let changes = events::reply(
            &mut patch,
            revision_ix,
            comment_id,
            &author,
            reply,
            Timestamp::now(),
        )?;

        let _cob = self
            .store
            .update(
                &self.whoami,
                project,
                UpdateObjectSpec {
                    object_id: *patch_id,
                    typename: TYPENAME.clone(),
                    message: Some("Reply".to_owned()),
                    changes,
                },
            )
            .unwrap();

        Ok(())
    }

    pub fn review(
        &self,
        project: &Urn,
        patch_id: &PatchId,
        revision_ix: RevisionIx,
        verdict: Option<Verdict>,
        comment: impl Into<String>,
        inline: Vec<CodeComment>,
    ) -> Result<(), Error> {
        let timestamp = Timestamp::now();
        let review = Review::new(self.author(), verdict, comment, inline, timestamp);

        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let (_, changes) = events::review(&mut patch, revision_ix, review)?;

        cobs::update(
            *patch_id,
            project,
            "Review patch",
            changes,
            &self.whoami,
            self.store,
        )?;

        Ok(())
    }

    pub fn get(&self, namespace: &Urn, id: &ObjectId) -> anyhow::Result<Option<Patch>> {
        self.store.get::<Patch>(namespace, id)
    }

    pub fn get_raw(&self, project: &Urn, id: &PatchId) -> Result<Option<Automerge>, Error> {
        let cob = self.store.retrieve(project, &TYPENAME, id)?;
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
        revision_ix: RevisionIx,
        commit: git::Oid,
    ) -> Result<Merge, Error> {
        let timestamp = Timestamp::now();
        let merge = Merge {
            peer: self.peer_id,
            commit,
            timestamp,
        };

        let mut patch = self.get_raw(project, patch_id)?.unwrap();
        let changes = events::merge(&mut patch, revision_ix, &merge)?;

        cobs::update(
            *patch_id,
            project,
            "Merge revision",
            changes,
            &self.whoami,
            self.store,
        )?;

        Ok(merge)
    }

    pub fn count(&self, project: &Urn) -> Result<usize, Error> {
        let cobs = self.store.list(project, &TYPENAME)?;

        Ok(cobs.len())
    }

    pub fn all(&self, project: &Urn) -> Result<Vec<(PatchId, Patch)>, Error> {
        let mut patches = Vec::new();
        let cobs = self.store.list(project, &TYPENAME)?;
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

impl<'a> FromValue<'a> for State {
    fn from_value(value: Value<'a>) -> Result<Self, ValueError> {
        let state = value.to_str().ok_or(ValueError::InvalidType)?;

        match state {
            "proposed" => Ok(Self::Proposed),
            "draft" => Ok(Self::Draft),
            "archived" => Ok(Self::Archived),
            _ => Err(ValueError::InvalidValue(value.to_string())),
        }
    }
}

/// A patch revision.
#[derive(Debug, Clone, Serialize)]
pub struct Revision<T = (), P = PeerId> {
    /// Unique revision ID. This is useful in case of conflicts, eg.
    /// a user published a revision from two devices by mistake.
    pub id: RevisionId,
    /// Peer who published this revision.
    pub peer: PeerId,
    /// Base branch commit (merge base).
    pub base: git::Oid,
    /// Reference to the Git object containing the code (revision head).
    pub oid: git::Oid,
    /// "Cover letter" for this changeset.
    pub comment: Comment,
    /// Discussion around this revision.
    pub discussion: Discussion,
    /// Reviews (one per user) of the changes.
    pub reviews: HashMap<Urn, Review>,
    /// Merges of this revision into other repositories.
    pub merges: Vec<Merge<P>>,
    /// Code changeset for this revision.
    pub changeset: T,
    /// When this revision was created.
    pub timestamp: Timestamp,
}

impl Revision {
    pub fn new(
        author: Author,
        peer: PeerId,
        base: git::Oid,
        oid: git::Oid,
        comment: String,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            peer,
            base,
            oid,
            comment: Comment::new(author, comment, timestamp),
            discussion: Discussion::default(),
            reviews: HashMap::default(),
            merges: Vec::default(),
            changeset: (),
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
        assert!(
            self.merges.is_empty(),
            "Cannot put revision with non-empty merges"
        );
        assert!(
            self.reviews.is_empty(),
            "Cannot put revision with non-empty reviews"
        );
        assert!(
            self.discussion.is_empty(),
            "Cannot put revision with non-empty discussion"
        );

        tx.put(&id, "id", self.id.to_string())?;
        tx.put(&id, "peer", self.peer.to_string())?;
        tx.put(&id, "oid", self.oid.to_string())?;
        tx.put(&id, "base", self.base.to_string())?;

        self.comment.put(tx, id)?;

        tx.put_object(&id, "discussion", ObjType::List)?;
        tx.put_object(&id, "reviews", ObjType::Map)?;
        tx.put_object(&id, "merges", ObjType::List)?;
        tx.put(&id, "timestamp", self.timestamp)?;

        Ok(())
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.comment.author.resolve(storage)?;
        for comment in &mut self.discussion {
            comment.resolve(storage)?;
        }
        for (_urn, review) in &mut self.reviews.iter_mut() {
            review.resolve(storage)?;
        }

        Ok(())
    }
}

/// A merged patch revision.
#[derive(Debug, Clone, Serialize)]
pub struct Merge<P = PeerId> {
    /// Peer id of repository that this patch was merged into.
    pub peer: P,
    /// Base branch commit that contains the revision.
    pub commit: git::Oid,
    /// When this merged was performed.
    pub timestamp: Timestamp,
}

/// A patch review verdict.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Accept patch.
    Accept,
    /// Reject patch.
    Reject,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Reject => write!(f, "reject"),
        }
    }
}

impl From<Verdict> for ScalarValue {
    fn from(verdict: Verdict) -> Self {
        let s = serde_json::to_string(&verdict).unwrap(); // Cannot fail.
        ScalarValue::from(s)
    }
}

impl<'a> FromValue<'a> for Verdict {
    fn from_value(value: Value) -> Result<Self, ValueError> {
        let verdict = value.to_str().ok_or(ValueError::InvalidType)?;
        serde_json::from_str(verdict).map_err(|e| ValueError::Other(Arc::new(e)))
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
    pub verdict: Option<Verdict>,
    /// Review general comment.
    pub comment: Comment<Replies>,
    /// Review inline code comments.
    pub inline: Vec<CodeComment>,
    /// Review timestamp.
    pub timestamp: Timestamp,
}

impl Review {
    pub fn new(
        author: Author,
        verdict: Option<Verdict>,
        comment: impl Into<String>,
        inline: Vec<CodeComment>,
        timestamp: Timestamp,
    ) -> Self {
        let comment = Comment::new(author.clone(), comment.into(), timestamp);

        Self {
            author,
            verdict,
            comment,
            inline,
            timestamp,
        }
    }

    /// Put this object into an automerge document.
    fn put(
        &self,
        tx: &mut automerge::transaction::Transaction,
        id: &automerge::ObjId,
    ) -> Result<(), AutomergeError> {
        assert!(
            self.inline.is_empty(),
            "Cannot put review with non-empty inline comments"
        );

        tx.put(&id, "author", self.author.urn().to_string())?;
        tx.put(&id, "peer", self.author.peer.default_encoding())?;
        tx.put(
            &id,
            "verdict",
            if let Some(v) = self.verdict {
                v.into()
            } else {
                ScalarValue::Null
            },
        )?;

        self.comment.put(tx, id)?;

        tx.put_object(&id, "inline", ObjType::List)?;
        tx.put(&id, "timestamp", self.timestamp)?;

        Ok(())
    }

    pub fn resolve<S: AsRef<ReadOnly>>(&mut self, storage: &S) -> Result<(), ResolveError> {
        self.author.resolve(storage)?;
        self.comment.resolve(storage)?;

        Ok(())
    }
}

mod lookup {
    use super::*;

    pub fn revision(
        doc: Document,
        revision_id: &automerge::ObjId,
    ) -> Result<Revision, DocumentError> {
        let (_, comment_id) = doc.get(&revision_id, "comment")?;
        let (_, reviews_id) = doc.get(&revision_id, "reviews")?;
        let id = doc.val(&revision_id, "id")?;
        let peer = doc.val(&revision_id, "peer")?;
        let base = doc.val(&revision_id, "base")?;
        let oid = doc.val(&revision_id, "oid")?;
        let timestamp = doc.val(&revision_id, "timestamp")?;

        let comment = shared::lookup::comment(doc, &comment_id)?;
        let discussion: Discussion =
            doc.list(&revision_id, "discussion", shared::lookup::thread)?;
        let merges: Vec<Merge> = doc.list(&revision_id, "merges", self::merge)?;

        // Reviews.
        let mut reviews: HashMap<Urn, Review> = HashMap::new();
        for key in (*doc).keys(&reviews_id) {
            let (_, review_id) = doc.get(&reviews_id, key)?;
            let review = self::review(doc, &review_id)?;

            reviews.insert(review.author.urn().clone(), review);
        }

        Ok(Revision {
            id,
            peer,
            base,
            oid,
            comment,
            discussion,
            reviews,
            merges,
            changeset: (),
            timestamp,
        })
    }

    pub fn merge(doc: Document, obj_id: &automerge::ObjId) -> Result<Merge, DocumentError> {
        let peer = doc.val(&obj_id, "peer")?;
        let commit = doc.val(&obj_id, "commit")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;

        Ok(Merge {
            peer,
            commit,
            timestamp,
        })
    }

    pub fn review(doc: Document, obj_id: &automerge::ObjId) -> Result<Review, DocumentError> {
        let author = doc.val(&obj_id, "author")?;
        let peer = doc.val(&obj_id, "peer")?;
        let verdict = doc.val(&obj_id, "verdict")?;
        let timestamp = doc.val(&obj_id, "timestamp")?;
        let comment = doc.lookup(&obj_id, "comment", shared::lookup::thread)?;
        let inline = vec![];

        Ok(Review {
            author: Author::new(author, peer),
            comment,
            verdict,
            inline,
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
        let cob = store.create(
            whoami,
            project,
            NewObjectSpec {
                typename: TYPENAME.clone(),
                message: Some("Create patch".to_owned()),
                history,
            },
        )?;

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
        let cob = store.update(
            whoami,
            project,
            UpdateObjectSpec {
                object_id,
                typename: TYPENAME.clone(),
                message: Some(message.to_owned()),
                changes,
            },
        )?;

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
        author: &Author,
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
                    tx.put(&patch_id, "author", author.urn().to_string())?;
                    tx.put(&patch_id, "peer", author.peer.default_encoding())?;
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

    pub fn comment(
        patch: &mut Automerge,
        revision_ix: RevisionIx,
        author: &Author,
        body: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        let _comment = patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Add comment".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();
                    let (_, revision_id) = tx.get(&revisions_id, revision_ix)?.unwrap();
                    let (_, discussion_id) = tx.get(&revision_id, "discussion")?.unwrap();

                    let length = tx.length(&discussion_id);
                    let comment = tx.insert_object(&discussion_id, length, ObjType::Map)?;

                    tx.put(&comment, "author", author.urn().to_string())?;
                    tx.put(&comment, "peer", author.peer.default_encoding())?;
                    tx.put(&comment, "body", body.trim())?;
                    tx.put(&comment, "timestamp", timestamp)?;
                    tx.put_object(&comment, "replies", ObjType::List)?;
                    tx.put_object(&comment, "reactions", ObjType::Map)?;

                    Ok(comment)
                },
            )
            .map_err(|failure| failure.error)?
            .result;

        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn update(
        patch: &mut Automerge,
        revision: Revision,
    ) -> Result<(RevisionIx, EntryContents), AutomergeError> {
        let success = patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Merge revision".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();

                    let ix = tx.length(&revisions_id);
                    let revision_id = tx.insert_object(&revisions_id, ix, ObjType::Map)?;

                    revision.put(tx, &revision_id)?;

                    Ok(ix)
                },
            )
            .map_err(|failure| failure.error)?;

        let revision_ix = success.result;
        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok((revision_ix, EntryContents::Automerge(change)))
    }

    pub fn reply(
        patch: &mut Automerge,
        revision_ix: RevisionIx,
        comment_id: CommentId,
        author: &Author,
        body: &str,
        timestamp: Timestamp,
    ) -> Result<EntryContents, AutomergeError> {
        patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Reply".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();
                    let (_, revision_id) = tx.get(&revisions_id, revision_ix)?.unwrap();
                    let (_, discussion_id) = tx.get(&revision_id, "discussion")?.unwrap();
                    let (_, comment_id) = tx.get(&discussion_id, usize::from(comment_id))?.unwrap();
                    let (_, replies_id) = tx.get(&comment_id, "replies")?.unwrap();

                    let length = tx.length(&replies_id);
                    let reply = tx.insert_object(&replies_id, length, ObjType::Map)?;

                    // Nb. Replies don't themselves have replies.
                    tx.put(&reply, "author", author.urn().to_string())?;
                    tx.put(&reply, "peer", author.peer.default_encoding())?;
                    tx.put(&reply, "body", body.trim())?;
                    tx.put(&reply, "timestamp", timestamp)?;
                    tx.put_object(&reply, "reactions", ObjType::Map)?;

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(EntryContents::Automerge(change))
    }

    pub fn review(
        patch: &mut Automerge,
        revision_ix: RevisionIx,
        review: Review,
    ) -> Result<((), EntryContents), AutomergeError> {
        patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Review patch".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();
                    let (_, revision_id) = tx.get(&revisions_id, revision_ix)?.unwrap();
                    let (_, reviews_id) = tx.get(&revision_id, "reviews")?.unwrap();

                    let review_id =
                        tx.put_object(&reviews_id, review.author.urn().to_string(), ObjType::Map)?;

                    review.put(tx, &review_id)?;

                    Ok(())
                },
            )
            .map_err(|failure| failure.error)?;

        let change = patch.get_last_local_change().unwrap().raw_bytes().to_vec();

        Ok(((), EntryContents::Automerge(change)))
    }

    pub fn merge(
        patch: &mut Automerge,
        revision_ix: RevisionIx,
        merge: &Merge,
    ) -> Result<EntryContents, AutomergeError> {
        patch
            .transact_with::<_, _, AutomergeError, _, ()>(
                |_| CommitOptions::default().with_message("Merge revision".to_owned()),
                |tx| {
                    let (_, obj_id) = tx.get(ObjId::Root, "patch")?.unwrap();
                    let (_, revisions_id) = tx.get(&obj_id, "revisions")?.unwrap();
                    let (_, revision_id) = tx.get(&revisions_id, revision_ix)?.unwrap();
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
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let patches = cobs.patches();
        let target = MergeTarget::Upstream;
        let oid = git::Oid::from(git2::Oid::zero());
        let base = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let patch_id = patches
            .create(
                &project.urn(),
                "My first patch",
                "Blah blah blah.",
                target,
                base,
                oid,
                &[],
            )
            .unwrap();
        let patch = patches.get(&project.urn(), &patch_id).unwrap().unwrap();

        assert_eq!(&patch.title, "My first patch");
        assert_eq!(patch.author.urn(), &author);
        assert_eq!(&patch.author.peer, storage.peer_id());
        assert_eq!(patch.state, State::Proposed);
        assert!(patch.timestamp >= timestamp);

        let revision = patch.revisions.head;

        assert_eq!(revision.peer, *storage.peer_id());
        assert_eq!(revision.comment.body, "Blah blah blah.");
        assert_eq!(revision.discussion.len(), 0);
        assert_eq!(revision.oid, oid);
        assert_eq!(revision.base, base);
        assert!(revision.reviews.is_empty());
        assert!(revision.merges.is_empty());
    }

    #[test]
    fn test_patch_merge() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let patches = cobs.patches();
        let target = MergeTarget::Upstream;
        let oid = git::Oid::from(git2::Oid::zero());
        let base = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let patch_id = patches
            .create(
                &project.urn(),
                "My first patch",
                "Blah blah blah.",
                target,
                base,
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
    fn test_patch_review() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami.clone(), profile.paths(), &storage);
        let patches = cobs.patches();
        let target = MergeTarget::Upstream;
        let base = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let rev_oid = git::Oid::from_str("518d5069f94c03427f694bb494ac1cd7d1339380").unwrap();
        let project = &project.urn();
        let patch_id = patches
            .create(
                project,
                "My first patch",
                "Blah blah blah.",
                target,
                base,
                rev_oid,
                &[],
            )
            .unwrap();

        patches
            .review(project, &patch_id, 0, Some(Verdict::Accept), "LGTM", vec![])
            .unwrap();
        let patch = patches.get(project, &patch_id).unwrap().unwrap();
        let reviews = patch.revisions.head.reviews;
        assert_eq!(reviews.len(), 1);

        let review = reviews.get(&whoami.urn()).unwrap();
        assert_eq!(review.author.urn(), &whoami.urn());
        assert_eq!(review.verdict, Some(Verdict::Accept));
        assert_eq!(review.comment.body.as_str(), "LGTM");
    }

    #[test]
    fn test_patch_update() {
        let (storage, profile, whoami, project) = test::setup::profile();
        let cobs = Store::new(whoami, profile.paths(), &storage);
        let patches = cobs.patches();
        let target = MergeTarget::Upstream;
        let base = git::Oid::from_str("af08e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let rev0_oid = git::Oid::from_str("518d5069f94c03427f694bb494ac1cd7d1339380").unwrap();
        let rev1_oid = git::Oid::from_str("cb18e95ada2bb38aadd8e6cef0963ce37a87add3").unwrap();
        let project = &project.urn();
        let patch_id = patches
            .create(
                project,
                "My first patch",
                "Blah blah blah.",
                target,
                base,
                rev0_oid,
                &[],
            )
            .unwrap();

        let patch = patches.get(project, &patch_id).unwrap().unwrap();
        assert_eq!(patch.description(), "Blah blah blah.");
        assert_eq!(patch.version(), 0);

        let revision_id = patches
            .update(project, &patch_id, "I've made changes.", base, rev1_oid)
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
